//! Native wgpu offscreen viewport (Vulkan/Metal/DX12) for mesh + smoke particles.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use elfentier_core::viewport::{ViewportLiquidFrame, ViewportMesh, ViewportSmokeFrame};
use pollster::block_on;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use wgpu::util::DeviceExt;

/// sRGB bytes for the wgpu clear color (0.047, 0.055, 0.071, 1.0).
pub const CLEAR_RGBA: [u8; 4] = [12, 14, 18, 255];

const MESH_SHADER: &str = r#"
struct Camera {
    view_proj: mat4x4f,
    light_dir: vec4f,
    eye: vec4f,
}

@group(0) @binding(0) var<uniform> camera: Camera;

struct VsIn {
    @location(0) position: vec3f,
    @location(1) normal: vec3f,
}

struct VsOut {
    @builtin(position) clip: vec4f,
    @location(0) normal: vec3f,
    @location(1) world: vec3f,
}

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.world = input.position;
    out.normal = input.normal;
    out.clip = camera.view_proj * vec4f(input.position, 1.0);
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4f {
    let n = normalize(input.normal);
    let l = normalize(camera.light_dir.xyz);
    let diff = max(dot(n, l), 0.0);
    let ambient = 0.18;
    let base = vec3f(0.56, 0.64, 0.77);
    let col = base * (ambient + diff * 0.82);
    return vec4f(col, 1.0);
}
"#;

const SMOKE_SHADER: &str = r#"
struct Camera {
    view_proj: mat4x4f,
    eye: vec4f,
    viewport: vec4f,
}

struct Particle {
    @location(0) center: vec3f,
    @location(1) size: f32,
    @location(2) opacity: f32,
}

struct VsOut {
    @builtin(position) clip: vec4f,
    @location(0) uv: vec2f,
    @location(1) opacity: f32,
}

@group(0) @binding(0) var<uniform> camera: Camera;

@vertex
fn vs_main(particle: Particle, @builtin(vertex_index) vid: u32) -> VsOut {
    let corners = array(
        vec2f(-1.0, -1.0),
        vec2f( 1.0, -1.0),
        vec2f(-1.0,  1.0),
        vec2f( 1.0, -1.0),
        vec2f( 1.0,  1.0),
        vec2f(-1.0,  1.0),
    );
    let uv = corners[vid];
    let to_particle = particle.center - camera.eye.xyz;
    let dist = length(to_particle);
    let world_up = vec3f(0.0, 1.0, 0.0);
    var right = normalize(cross(world_up, normalize(to_particle)));
    if (length(right) < 0.001) {
        right = vec3f(1.0, 0.0, 0.0);
    }
    let up = cross(normalize(to_particle), right);
    let radius = particle.size * (0.45 + 0.55 * particle.opacity);
    let offset = right * uv.x * radius + up * uv.y * radius;
    let world = particle.center + offset;
    var out: VsOut;
    out.clip = camera.view_proj * vec4f(world, 1.0);
    out.uv = uv;
    out.opacity = particle.opacity * 0.55;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4f {
    let r = length(input.uv);
    if (r > 1.0) {
        discard;
    }
    let falloff = pow(1.0 - r, 2.2);
    let col = vec3f(0.86, 0.89, 0.94);
    return vec4f(col, input.opacity * falloff);
}
"#;

const LIQUID_SHADER: &str = r#"
struct Camera {
    view_proj: mat4x4f,
    eye: vec4f,
    viewport: vec4f,
}

struct Particle {
    @location(0) center: vec3f,
    @location(1) radius: f32,
    @location(2) opacity: f32,
}

struct VsOut {
    @builtin(position) clip: vec4f,
    @location(0) uv: vec2f,
    @location(1) opacity: f32,
}

@group(0) @binding(0) var<uniform> camera: Camera;

@vertex
fn vs_main(particle: Particle, @builtin(vertex_index) vid: u32) -> VsOut {
    let corners = array(
        vec2f(-1.0, -1.0),
        vec2f( 1.0, -1.0),
        vec2f(-1.0,  1.0),
        vec2f( 1.0, -1.0),
        vec2f( 1.0,  1.0),
        vec2f(-1.0,  1.0),
    );
    let uv = corners[vid];
    let to_particle = particle.center - camera.eye.xyz;
    var right = normalize(cross(vec3f(0.0, 1.0, 0.0), normalize(to_particle)));
    if (length(right) < 0.001) {
        right = vec3f(1.0, 0.0, 0.0);
    }
    let up = cross(normalize(to_particle), right);
    let radius = particle.radius * (0.7 + 0.3 * particle.opacity);
    let offset = right * uv.x * radius + up * uv.y * radius;
    let world = particle.center + offset;
    var out: VsOut;
    out.clip = camera.view_proj * vec4f(world, 1.0);
    out.uv = uv;
    out.opacity = particle.opacity * 0.75;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4f {
    let r = length(input.uv);
    if (r > 1.0) {
        discard;
    }
    let falloff = pow(1.0 - r, 2.5);
    let deep = vec3f(0.08, 0.28, 0.62);
    let foam = vec3f(0.45, 0.72, 0.95);
    let col = mix(deep, foam, falloff * 0.65 + 0.2);
    return vec4f(col, input.opacity * falloff);
}
"#;

const COLLIDER_SHADER: &str = r#"
struct Camera {
    view_proj: mat4x4f,
}

@group(0) @binding(0) var<uniform> camera: Camera;

struct VsIn {
    @location(0) position: vec3f,
}

@vertex
fn vs_main(input: VsIn) -> @builtin(position) vec4f {
    return camera.view_proj * vec4f(input.position, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4f {
    return vec4f(0.95, 0.62, 0.22, 1.0);
}
"#;

const GRID_SHADER: &str = r#"
struct Camera {
    view_proj: mat4x4f,
}

@group(0) @binding(0) var<uniform> camera: Camera;

struct VsIn {
    @location(0) position: vec3f,
}

@vertex
fn vs_main(input: VsIn) -> @builtin(position) vec4f {
    return camera.view_proj * vec4f(input.position, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4f {
    return vec4f(0.16, 0.19, 0.24, 1.0);
}
"#;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NativeViewportCamera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_y_deg: f32,
}

impl Default for NativeViewportCamera {
    fn default() -> Self {
        Self {
            eye: [28.0, 22.0, 36.0],
            target: [24.0, 4.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_y_deg: 50.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativePreviewImage {
    pub width: u32,
    pub height: u32,
    /// Base64-encoded RGBA8 pixels (`width * height * 4` bytes decoded).
    pub rgba_base64: String,
    pub backend: String,
}

impl NativePreviewImage {
    pub fn from_rgba(
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        backend: impl Into<String>,
    ) -> Result<Self, String> {
        let expected = width as usize * height as usize * 4;
        if rgba.len() != expected {
            return Err(format!(
                "preview rgba length {} does not match {}x{} (expected {} bytes)",
                rgba.len(),
                width,
                height,
                expected
            ));
        }
        Ok(Self {
            width,
            height,
            rgba_base64: STANDARD.encode(&rgba),
            backend: backend.into(),
        })
    }

    pub fn decode_rgba(&self) -> Result<Vec<u8>, String> {
        let expected = self.width as usize * self.height as usize * 4;
        let rgba = STANDARD
            .decode(&self.rgba_base64)
            .map_err(|err| format!("preview rgba base64 decode failed: {err}"))?;
        if rgba.len() != expected {
            return Err(format!(
                "decoded preview rgba length {} does not match {}x{} (expected {} bytes)",
                rgba.len(),
                self.width,
                self.height,
                expected
            ));
        }
        Ok(rgba)
    }
}

/// Returns true when the buffer contains pixels that differ from the wgpu clear color.
pub fn preview_has_visible_pixels(rgba: &[u8], clear: [u8; 4]) -> bool {
    rgba.chunks_exact(4).any(|px| px != clear)
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshVertex {
    position: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
    eye: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SmokeCameraUniform {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    viewport: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SmokeParticle {
    center: [f32; 3],
    size: f32,
    opacity: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LiquidParticle {
    center: [f32; 3],
    radius: f32,
    opacity: f32,
    _pad: f32,
}

/// Default camera fitted to a cooked viewport payload.
pub fn default_camera_for_mesh(mesh: &ViewportMesh) -> NativeViewportCamera {
    let (min, max) = bounds_for_mesh(mesh);
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let extent = [
        max[0] - min[0],
        max[1] - min[1],
        max[2] - min[2],
    ]
    .into_iter()
    .fold(6.0_f32, f32::max);
    let dist = extent * 1.8;
    NativeViewportCamera {
        eye: [
            center[0] + dist * 0.65,
            center[1] + dist * 0.5,
            center[2] + dist * 0.75,
        ],
        target: center,
        up: [0.0, 1.0, 0.0],
        fov_y_deg: 50.0,
    }
}

/// Renders mesh and/or smoke particles with native wgpu.
pub fn render_native_viewport(
    mesh: &ViewportMesh,
    smoke_frame: u32,
    width: u32,
    height: u32,
    camera: &NativeViewportCamera,
) -> Result<NativePreviewImage, String> {
    match block_on(render_wgpu(mesh, smoke_frame, width, height, camera)) {
        Ok(image) => Ok(image),
        Err(err) => Err(format!("wgpu render failed: {err}")),
    }
}

fn bounds_for_mesh(mesh: &ViewportMesh) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];

    if mesh.vertex_count > 0 {
        for inst_idx in 0..mesh.instance_count.max(1) {
            let m = matrix_at(&mesh.instance_matrices, inst_idx);
            for v in 0..mesh.positions.len() / 3 {
                let p = transform_point(
                    m,
                    [
                        mesh.positions[v * 3],
                        mesh.positions[v * 3 + 1],
                        mesh.positions[v * 3 + 2],
                    ],
                );
                for axis in 0..3 {
                    min[axis] = min[axis].min(p[axis]);
                    max[axis] = max[axis].max(p[axis]);
                }
            }
        }
    }

    if let Some(smoke) = &mesh.smoke {
        for axis in 0..3 {
            min[axis] = min[axis].min(smoke.bounds_min[axis]);
            max[axis] = max[axis].max(smoke.bounds_max[axis]);
        }
    }

    if let Some(liquid) = &mesh.liquid {
        for axis in 0..3 {
            min[axis] = min[axis].min(liquid.bounds_min[axis]);
            max[axis] = max[axis].max(liquid.bounds_max[axis]);
        }
    }

    for collider in &mesh.colliders {
        for axis in 0..3 {
            min[axis] = min[axis].min(collider.bounds_min[axis]);
            max[axis] = max[axis].max(collider.bounds_max[axis]);
        }
    }

    if min[0].is_finite() {
        (min, max)
    } else {
        ([-4.0, 0.0, -4.0], [4.0, 8.0, 4.0])
    }
}

fn matrix_at(matrices: &[f32], index: u32) -> [f32; 16] {
    let i = index as usize * 16;
    let mut out = [0.0; 16];
    if i + 16 <= matrices.len() {
        out.copy_from_slice(&matrices[i..i + 16]);
    } else {
        out[0] = 1.0;
        out[5] = 1.0;
        out[10] = 1.0;
        out[15] = 1.0;
    }
    out
}

fn transform_point(m: [f32; 16], p: [f32; 3]) -> [f32; 3] {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

fn merge_mesh(mesh: &ViewportMesh) -> (Vec<MeshVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    if mesh.positions.is_empty() || mesh.indices.is_empty() {
        return (vertices, indices);
    }

    let base_normals = compute_vertex_normals(&mesh.positions, &mesh.indices);
    let inst_count = mesh.instance_count.max(1);

    for inst in 0..inst_count {
        let m = matrix_at(&mesh.instance_matrices, inst);
        let base = vertices.len() as u32;
        for (vi, normal) in base_normals.iter().enumerate() {
            let p = [
                mesh.positions[vi * 3],
                mesh.positions[vi * 3 + 1],
                mesh.positions[vi * 3 + 2],
            ];
            let n = transform_normal(m, *normal);
            vertices.push(MeshVertex {
                position: transform_point(m, p),
                normal: n,
            });
        }
        for idx in &mesh.indices {
            indices.push(base + idx);
        }
    }

    (vertices, indices)
}

fn transform_normal(m: [f32; 16], n: [f32; 3]) -> [f32; 3] {
    let tn = [
        m[0] * n[0] + m[4] * n[1] + m[8] * n[2],
        m[1] * n[0] + m[5] * n[1] + m[9] * n[2],
        m[2] * n[0] + m[6] * n[1] + m[10] * n[2],
    ];
    normalize3(tn)
}

fn compute_vertex_normals(positions: &[f32], indices: &[u32]) -> Vec<[f32; 3]> {
    let vert_count = positions.len() / 3;
    let mut normals = vec![[0.0_f32; 3]; vert_count];
    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let a = vertex_at(positions, tri[0] as usize);
        let b = vertex_at(positions, tri[1] as usize);
        let c = vertex_at(positions, tri[2] as usize);
        let e1 = sub3(b, a);
        let e2 = sub3(c, a);
        let face = normalize3(cross3(e1, e2));
        for &idx in tri {
            let n = &mut normals[idx as usize];
            n[0] += face[0];
            n[1] += face[1];
            n[2] += face[2];
        }
    }
    normals
        .into_iter()
        .map(|n| {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 1e-6 {
                [n[0] / len, n[1] / len, n[2] / len]
            } else {
                [0.0, 1.0, 0.0]
            }
        })
        .collect()
}

fn vertex_at(positions: &[f32], index: usize) -> [f32; 3] {
    [
        positions[index * 3],
        positions[index * 3 + 1],
        positions[index * 3 + 2],
    ]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn build_grid_lines(min: [f32; 3], max: [f32; 3]) -> Vec<[f32; 3]> {
    let y = min[1].min(0.0);
    let span = [
        max[0] - min[0],
        max[2] - min[2],
    ]
    .into_iter()
    .fold(12.0_f32, f32::max)
    .max(12.0);
    let cx = (min[0] + max[0]) * 0.5;
    let cz = (min[2] + max[2]) * 0.5;
    let half = span * 0.6;
    let steps = 12;
    let mut lines = Vec::new();
    for i in 0..=steps {
        let t = -half + (2.0 * half * i as f32 / steps as f32);
        lines.push([cx + t, y, cz - half]);
        lines.push([cx + t, y, cz + half]);
        lines.push([cx - half, y, cz + t]);
        lines.push([cx + half, y, cz + t]);
    }
    lines
}

fn look_at_rh(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = normalize3([target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]]);
    let s = normalize3(cross3(f, up));
    let u = cross3(s, f);
    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [-dot3(s, eye), -dot3(u, eye), dot3(f, eye), 1.0],
    ]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn perspective_rh(fov_y_deg: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (0.5 * fov_y_deg.to_radians()).tan();
    let nf = 1.0 / (near - far);
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far * nf, -1.0],
        [0.0, 0.0, near * far * nf, 0.0],
    ]
}

fn mul4(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            out[r][c] = a[r][0] * b[0][c]
                + a[r][1] * b[1][c]
                + a[r][2] * b[2][c]
                + a[r][3] * b[3][c];
        }
    }
    out
}

fn pack_liquid_particles(frame: &ViewportLiquidFrame) -> Vec<LiquidParticle> {
    let count = frame.particle_count as usize;
    (0..count)
        .map(|i| LiquidParticle {
            center: [
                frame.positions[i * 3],
                frame.positions[i * 3 + 1],
                frame.positions[i * 3 + 2],
            ],
            radius: frame.radii.get(i).copied().unwrap_or(0.12),
            opacity: frame.opacities.get(i).copied().unwrap_or(0.7),
            _pad: 0.0,
        })
        .collect()
}

fn pack_particles(frame: &ViewportSmokeFrame) -> Vec<SmokeParticle> {
    let count = frame.particle_count as usize;
    (0..count)
        .map(|i| SmokeParticle {
            center: [
                frame.positions[i * 3],
                frame.positions[i * 3 + 1],
                frame.positions[i * 3 + 2],
            ],
            size: frame.sizes.get(i).copied().unwrap_or(0.25),
            opacity: frame.opacities.get(i).copied().unwrap_or(0.5),
            _pad: 0.0,
        })
        .collect()
}

async fn render_wgpu(
    mesh: &ViewportMesh,
    smoke_frame: u32,
    width: u32,
    height: u32,
    camera: &NativeViewportCamera,
) -> Result<NativePreviewImage, String> {
    let w = width.max(64).min(1920);
    let h = height.max(64).min(1080);

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or("no wgpu adapter")?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("elfentier native viewport"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )
        .await
        .map_err(|e| e.to_string())?;

    let view = look_at_rh(camera.eye, camera.target, camera.up);
    let proj = perspective_rh(camera.fov_y_deg, w as f32 / h as f32, 0.1, 500.0);
    let view_proj = mul4(proj, view);

    let mesh_camera = CameraUniform {
        view_proj,
        light_dir: [0.45, 0.85, 0.25, 0.0],
        eye: [camera.eye[0], camera.eye[1], camera.eye[2], 1.0],
    };

    let smoke_camera = SmokeCameraUniform {
        view_proj,
        eye: [camera.eye[0], camera.eye[1], camera.eye[2], 1.0],
        viewport: [w as f32, h as f32, 0.0, 0.0],
    };

    let (min, max) = bounds_for_mesh(mesh);
    let (mesh_vertices, mesh_indices) = merge_mesh(mesh);
    let grid_lines = build_grid_lines(min, max);
    let mut collider_lines: Vec<[f32; 3]> = Vec::new();
    for collider in &mesh.colliders {
        for chunk in collider.lines.chunks(3) {
            if chunk.len() == 3 {
                collider_lines.push([chunk[0], chunk[1], chunk[2]]);
            }
        }
    }

    let output_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("native viewport color"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let output_view = output_texture.create_view(&Default::default());

    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("native viewport depth"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&Default::default());

    let mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mesh shader"),
        source: wgpu::ShaderSource::Wgsl(MESH_SHADER.into()),
    });
    let smoke_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("smoke shader"),
        source: wgpu::ShaderSource::Wgsl(SMOKE_SHADER.into()),
    });
    let liquid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("liquid shader"),
        source: wgpu::ShaderSource::Wgsl(LIQUID_SHADER.into()),
    });
    let grid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("grid shader"),
        source: wgpu::ShaderSource::Wgsl(GRID_SHADER.into()),
    });
    let collider_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("collider shader"),
        source: wgpu::ShaderSource::Wgsl(COLLIDER_SHADER.into()),
    });

    let mesh_camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("mesh camera"),
        contents: bytemuck::bytes_of(&mesh_camera),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let smoke_camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("smoke camera"),
        contents: bytemuck::bytes_of(&smoke_camera),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let mesh_bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mesh bind layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let mesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mesh bind group"),
        layout: &mesh_bind_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: mesh_camera_buffer.as_entire_binding(),
        }],
    });

    let smoke_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("smoke bind group"),
        layout: &mesh_bind_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: smoke_camera_buffer.as_entire_binding(),
        }],
    });

    let grid_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("grid bind group"),
        layout: &mesh_bind_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: mesh_camera_buffer.as_entire_binding(),
        }],
    });

    let mesh_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mesh pipeline layout"),
        bind_group_layouts: &[&mesh_bind_layout],
        push_constant_ranges: &[],
    });

    let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mesh pipeline"),
        layout: Some(&mesh_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &mesh_shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<MeshVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 12,
                        shader_location: 1,
                    },
                ],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &mesh_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("grid pipeline"),
        layout: Some(&mesh_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &grid_shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 12,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &grid_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let collider_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("collider pipeline"),
        layout: Some(&mesh_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &collider_shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 12,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &collider_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let liquid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("liquid pipeline"),
        layout: Some(&mesh_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &liquid_shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<LiquidParticle>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32,
                        offset: 12,
                        shader_location: 1,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32,
                        offset: 16,
                        shader_location: 2,
                    },
                ],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &liquid_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let smoke_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("smoke pipeline"),
        layout: Some(&mesh_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &smoke_shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<SmokeParticle>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32,
                        offset: 12,
                        shader_location: 1,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32,
                        offset: 16,
                        shader_location: 2,
                    },
                ],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &smoke_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let mesh_vertex_buffer = if mesh_vertices.is_empty() {
        None
    } else {
        Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh vertices"),
            contents: bytemuck::cast_slice(&mesh_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }))
    };

    let mesh_index_buffer = if mesh_indices.is_empty() {
        None
    } else {
        Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh indices"),
            contents: bytemuck::cast_slice(&mesh_indices),
            usage: wgpu::BufferUsages::INDEX,
        }))
    };

    let grid_flat: Vec<f32> = grid_lines.iter().flat_map(|p| p.iter().copied()).collect();
    let grid_vertex_buffer = if grid_flat.is_empty() {
        None
    } else {
        Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid lines"),
            contents: bytemuck::cast_slice(&grid_flat),
            usage: wgpu::BufferUsages::VERTEX,
        }))
    };

    let collider_flat: Vec<f32> = collider_lines.iter().flat_map(|p| p.iter().copied()).collect();
    let collider_vertex_buffer = if collider_flat.is_empty() {
        None
    } else {
        Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("collider lines"),
            contents: bytemuck::cast_slice(&collider_flat),
            usage: wgpu::BufferUsages::VERTEX,
        }))
    };

    let smoke_particles = mesh
        .smoke
        .as_ref()
        .and_then(|s| s.frames.get(smoke_frame as usize))
        .map(pack_particles);

    let smoke_instance_buffer = smoke_particles.as_ref().map(|particles| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("smoke particles"),
            contents: bytemuck::cast_slice(particles),
            usage: wgpu::BufferUsages::VERTEX,
        })
    });

    let liquid_particles = mesh
        .liquid
        .as_ref()
        .and_then(|l| l.frames.get(smoke_frame as usize))
        .map(pack_liquid_particles);

    let liquid_instance_buffer = liquid_particles.as_ref().map(|particles| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("liquid particles"),
            contents: bytemuck::cast_slice(particles),
            usage: wgpu::BufferUsages::VERTEX,
        })
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("native viewport encoder"),
    });

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("native viewport pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.047,
                        g: 0.055,
                        b: 0.071,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if let (Some(grid_vb), Some(grid_bg)) = (&grid_vertex_buffer, Some(&grid_bind_group)) {
            pass.set_pipeline(&grid_pipeline);
            pass.set_bind_group(0, grid_bg, &[]);
            pass.set_vertex_buffer(0, grid_vb.slice(..));
            pass.draw(0..grid_lines.len() as u32, 0..1);
        }

        if let (Some(collider_vb), Some(collider_bg)) =
            (&collider_vertex_buffer, Some(&grid_bind_group))
        {
            pass.set_pipeline(&collider_pipeline);
            pass.set_bind_group(0, collider_bg, &[]);
            pass.set_vertex_buffer(0, collider_vb.slice(..));
            pass.draw(0..collider_lines.len() as u32, 0..1);
        }

        if let (Some(vb), Some(ib), Some(bg)) = (
            &mesh_vertex_buffer,
            &mesh_index_buffer,
            Some(&mesh_bind_group),
        ) {
            pass.set_pipeline(&mesh_pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh_indices.len() as u32, 0, 0..1);
        }

        if let (Some(vb), Some(bg)) = (&liquid_instance_buffer, Some(&smoke_bind_group)) {
            let count = liquid_particles.as_ref().map(|p| p.len()).unwrap_or(0) as u32;
            if count > 0 {
                pass.set_pipeline(&liquid_pipeline);
                pass.set_bind_group(0, bg, &[]);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.draw(0..6, 0..count);
            }
        }

        if let (Some(vb), Some(bg)) = (&smoke_instance_buffer, Some(&smoke_bind_group)) {
            let count = smoke_particles.as_ref().map(|p| p.len()).unwrap_or(0) as u32;
            if count > 0 {
                pass.set_pipeline(&smoke_pipeline);
                pass.set_bind_group(0, bg, &[]);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.draw(0..6, 0..count);
            }
        }
    }

    let bytes_per_row = wgpu::util::align_to(w * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer_size = bytes_per_row as u64 * h as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("native viewport readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &output_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (map_tx, map_rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |status| {
        let _ = map_tx.send(status);
    });
    device.poll(wgpu::Maintain::Wait);
    match map_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("wgpu readback map failed: {err}")),
        Err(_) => return Err("wgpu readback map channel closed".into()),
    }

    let mapped = slice.get_mapped_range();
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for row in 0..h as usize {
        let src_start = row * bytes_per_row as usize;
        let dst_start = row * w as usize * 4;
        rgba[dst_start..dst_start + w as usize * 4]
            .copy_from_slice(&mapped[src_start..src_start + w as usize * 4]);
    }
    drop(mapped);
    readback.unmap();

    NativePreviewImage::from_rgba(w, h, rgba, "wgpu")
}

#[cfg(test)]
mod tests {
    use super::*;
    use elfentier_core::graph::Graph;
    use elfentier_core::viewport::cook_viewport_mesh;

    #[test]
    fn default_camera_is_finite() {
        let mesh = cook_viewport_mesh(&Graph::smoke_puff_preset()).expect("cook");
        let cam = default_camera_for_mesh(&mesh);
        assert!(cam.eye.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn default_camera_finite_for_liquid() {
        let mesh = cook_viewport_mesh(&Graph::ocean_patch_preset()).expect("cook");
        let cam = default_camera_for_mesh(&mesh);
        assert!(cam.eye.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn merge_mesh_produces_geometry_for_city() {
        let mesh = cook_viewport_mesh(&Graph::shop_street_preset()).expect("cook");
        let (verts, indices) = merge_mesh(&mesh);
        assert!(!verts.is_empty());
        assert!(!indices.is_empty());
    }

    #[test]
    fn preview_from_rgba_validates_length() {
        let err = NativePreviewImage::from_rgba(2, 2, vec![0, 1, 2], "wgpu")
            .expect_err("short buffer");
        assert!(err.contains("expected 16 bytes"));
    }

    #[test]
    fn preview_base64_round_trip() {
        let rgba = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let preview = NativePreviewImage::from_rgba(2, 2, rgba.clone(), "wgpu").expect("pack");
        assert_eq!(preview.decode_rgba().expect("decode"), rgba);
    }

    #[test]
    fn render_smoke_puff_has_non_clear_pixels() {
        let mesh = cook_viewport_mesh(&Graph::smoke_puff_preset()).expect("cook");
        let camera = default_camera_for_mesh(&mesh);
        let preview = render_native_viewport(&mesh, 0, 320, 240, &camera).expect("render");
        let rgba = preview.decode_rgba().expect("decode");
        assert_eq!(rgba.len(), 320 * 240 * 4);
        assert!(
            preview_has_visible_pixels(&rgba, CLEAR_RGBA),
            "smoke_puff preview should contain grid, collider, or smoke pixels"
        );
    }

    #[test]
    fn render_smoke_sphere_has_non_clear_pixels() {
        let mesh = cook_viewport_mesh(&Graph::smoke_sphere_preset()).expect("cook");
        let camera = default_camera_for_mesh(&mesh);
        let preview = render_native_viewport(&mesh, 0, 320, 240, &camera).expect("render");
        let rgba = preview.decode_rgba().expect("decode");
        assert!(
            preview_has_visible_pixels(&rgba, CLEAR_RGBA),
            "smoke_sphere preview should contain mesh collider and smoke pixels"
        );
    }

    #[test]
    fn render_shop_street_has_non_clear_pixels() {
        let mesh = cook_viewport_mesh(&Graph::shop_street_preset()).expect("cook");
        let camera = default_camera_for_mesh(&mesh);
        let preview = render_native_viewport(&mesh, 0, 320, 240, &camera).expect("render");
        let rgba = preview.decode_rgba().expect("decode");
        assert!(
            preview_has_visible_pixels(&rgba, CLEAR_RGBA),
            "shop_street preview should contain building mesh and grid pixels"
        );
    }
}
