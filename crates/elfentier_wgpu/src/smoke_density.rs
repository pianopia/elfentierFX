//! GPU volume raymarch for smoke viewport previews (trilinear density + Beer-Lambert).

/// WGSL shader: full-screen raymarch through a 3D density grid.
pub const VOLUME_RAYMARCH_SHADER: &str = r#"
struct VolumeUniform {
    eye: vec4f,
    forward: vec4f,
    right: vec4f,
    up: vec4f,
    viewport: vec4f,
    bounds_min: vec4f,
    bounds_max: vec4f,
    resolution: vec4f,
    density_params: vec4f,
    light_dir: vec4f,
    clear_color: vec4f,
}

@group(0) @binding(0) var<uniform> vol: VolumeUniform;
@group(0) @binding(1) var density_tex: texture_3d<f32>;

struct VsOut {
    @builtin(position) clip: vec4f,
    @location(0) ndc: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    let positions = array(
        vec2f(-1.0, -1.0),
        vec2f( 3.0, -1.0),
        vec2f(-1.0,  3.0),
    );
    let pos = positions[vid];
    var out: VsOut;
    out.clip = vec4f(pos, 0.0, 1.0);
    out.ndc = pos;
    return out;
}

fn ray_box_intersect(ro: vec3f, rd: vec3f, bmin: vec3f, bmax: vec3f) -> vec2f {
    let inv_rd = 1.0 / rd;
    let t0 = (bmin - ro) * inv_rd;
    let t1 = (bmax - ro) * inv_rd;
    let tmin = max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z));
    let tmax = min(min(max(t0.x, t1.x), max(t0.y, t1.y)), max(t0.z, t1.z));
    if (tmax < tmin) {
        return vec2f(-1.0, -1.0);
    }
    return vec2f(max(tmin, 0.0), tmax);
}

fn sample_density_trilinear(world_pos: vec3f) -> f32 {
    let bmin = vol.bounds_min.xyz;
    let bmax = vol.bounds_max.xyz;
    let res = vol.resolution.xyz;
    let extent = bmax - bmin;
    let uvw = (world_pos - bmin) / extent;
    if (any(uvw < vec3f(0.0)) || any(uvw > vec3f(1.0))) {
        return 0.0;
    }
    let coord = uvw * res - vec3f(0.5);
    let i0 = floor(coord);
    let frac = coord - i0;
    var sum = 0.0;
    for (var dz = 0; dz <= 1; dz++) {
        for (var dy = 0; dy <= 1; dy++) {
            for (var dx = 0; dx <= 1; dx++) {
                let idx = vec3u(
                    u32(clamp(i0.x + f32(dx), 0.0, res.x - 1.0)),
                    u32(clamp(i0.y + f32(dy), 0.0, res.y - 1.0)),
                    u32(clamp(i0.z + f32(dz), 0.0, res.z - 1.0)),
                );
                let wx = select(1.0 - frac.x, frac.x, dx == 1);
                let wy = select(1.0 - frac.y, frac.y, dy == 1);
                let wz = select(1.0 - frac.z, frac.z, dz == 1);
                sum += textureLoad(density_tex, idx, 0).r * wx * wy * wz;
            }
        }
    }
    return sum;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4f {
    let aspect = vol.viewport.x;
    let tan_half = vol.viewport.y;
    let rd = normalize(
        vol.forward.xyz
            + vol.right.xyz * input.ndc.x * tan_half * aspect
            + vol.up.xyz * input.ndc.y * tan_half,
    );
    let ro = vol.eye.xyz;

    let bmin = vol.bounds_min.xyz;
    let bmax = vol.bounds_max.xyz;
    let hit = ray_box_intersect(ro, rd, bmin, bmax);
    if (hit.x < 0.0) {
        discard;
    }

    let res = vol.resolution.xyz;
    let extent = bmax - bmin;
    let max_res = max(res.x, max(res.y, res.z));
    let step_size = length(extent) / max_res * 0.42;
    let max_steps = 192u;
    let density_scale = vol.density_params.x;
    let absorption = vol.density_params.y;
    let scatter = vol.density_params.z;
    let ambient = vol.density_params.w;
    let light_dir = normalize(vol.light_dir.xyz);

    var transmittance = 1.0;
    var accumulated = vec3f(0.0);
    var t = hit.x;
    let t_end = hit.y;
    var steps = 0u;

    while (t < t_end && steps < max_steps && transmittance > 0.02) {
        let pos = ro + rd * t;
        let d = sample_density_trilinear(pos) * density_scale;
        if (d > 0.001) {
            let sigma_t = d * absorption;
            let sigma_s = d * scatter;
            let cos_theta = dot(rd, -light_dir);
            let phase = 0.35 + 0.65 * max(cos_theta, 0.0);
            let light_contrib = ambient + phase;
            let absorb = exp(-sigma_t * step_size);
            accumulated += transmittance * sigma_s * light_contrib * step_size;
            transmittance *= absorb;
        }
        t += step_size;
        steps += 1u;
    }

    if (transmittance > 0.995) {
        discard;
    }

    let density_norm = clamp(length(accumulated) * 3.5, 0.0, 1.0);
    let warm = vec3f(0.96, 0.94, 0.90);
    let dense = vec3f(0.70, 0.68, 0.64);
    let cool = vec3f(0.82, 0.86, 0.92);
    let temp_tint = mix(warm, cool, clamp(vol.light_dir.w, 0.0, 1.0));
    let col = mix(temp_tint, dense, density_norm);
    let alpha = clamp((1.0 - transmittance) * 1.15, 0.0, 0.92);
    return vec4f(col, alpha);
}
"#;

/// Uniform block for the volume raymarch pass (must match WGSL layout).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VolumeUniform {
    pub eye: [f32; 4],
    pub forward: [f32; 4],
    pub right: [f32; 4],
    pub up: [f32; 4],
    pub viewport: [f32; 4],
    pub bounds_min: [f32; 4],
    pub bounds_max: [f32; 4],
    pub resolution: [f32; 4],
    pub density_params: [f32; 4],
    pub light_dir: [f32; 4],
    pub clear_color: [f32; 4],
}

/// Builds volume uniform data from density grid metadata and camera basis.
pub fn build_volume_uniform(
    eye: [f32; 3],
    forward: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    aspect: f32,
    tan_half_fov: f32,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    resolution: [u32; 3],
    max_density: f32,
    clear_rgb: [f32; 3],
    temperature: f32,
) -> VolumeUniform {
    let max_d = max_density.max(1e-5);
    VolumeUniform {
        eye: [eye[0], eye[1], eye[2], 1.0],
        forward: [forward[0], forward[1], forward[2], 0.0],
        right: [right[0], right[1], right[2], 0.0],
        up: [up[0], up[1], up[2], 0.0],
        viewport: [aspect, tan_half_fov, 0.0, 0.0],
        bounds_min: [bounds_min[0], bounds_min[1], bounds_min[2], 0.0],
        bounds_max: [bounds_max[0], bounds_max[1], bounds_max[2], 0.0],
        resolution: [
            resolution[0].max(1) as f32,
            resolution[1].max(1) as f32,
            resolution[2].max(1) as f32,
            0.0,
        ],
        density_params: [4.0 / max_d, 2.4, 1.6, 0.38],
        light_dir: [0.35, 0.82, 0.45, temperature.clamp(0.0, 1.0)],
        clear_color: [clear_rgb[0], clear_rgb[1], clear_rgb[2], 1.0],
    }
}

/// Camera basis vectors for ray construction (matches viewport camera math).
pub fn camera_basis(
    eye: [f32; 3],
    target: [f32; 3],
    up_hint: [f32; 3],
) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let forward = normalize3([
        target[0] - eye[0],
        target[1] - eye[1],
        target[2] - eye[2],
    ]);
    let right = normalize3(cross3(up_hint, forward));
    let up = cross3(forward, right);
    (forward, right, up)
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
