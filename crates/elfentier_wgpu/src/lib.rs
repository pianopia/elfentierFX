//! Native wgpu offscreen viewport (Vulkan/Metal/DX12) for mesh + smoke density previews.

mod smoke_density;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use elfentier_core::viewport::{ViewportLiquidFrame, ViewportMesh, ViewportSmokeFrame};
use pollster::block_on;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use wgpu::util::DeviceExt;

/// Linear-clear reference used in docs; wgpu Rgba8UnormSrgb readback differs.
pub const CLEAR_RGBA: [u8; 4] = [12, 14, 18, 255];

/// Observed framebuffer clear bytes from wgpu `Rgba8UnormSrgb` readback.
pub const GPU_CLEAR_RGBA: [u8; 4] = [61, 66, 75, 255];

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
    let to_eye = camera.eye.xyz - particle.center;
    let forward = to_eye / max(length(to_eye), 0.2);
    let world_up = vec3f(0.0, 1.0, 0.0);
    var right = normalize(cross(world_up, forward));
    if (length(right) < 0.001) {
        right = vec3f(1.0, 0.0, 0.0);
    }
    let up = cross(forward, right);
    let puff_scale = 0.82 + 0.38 * particle.opacity;
    let radius = particle.size * puff_scale;
    let offset = right * uv.x * radius + up * uv.y * radius;
    let world = particle.center + offset;
    var out: VsOut;
    out.clip = camera.view_proj * vec4f(world, 1.0);
    out.uv = uv;
    out.opacity = clamp(particle.opacity, 0.15, 1.0);
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4f {
    let r = length(input.uv);
    if (r > 1.12) {
        discard;
    }
    let edge = smoothstep(1.12, 0.72, r);
    let core = exp(-r * r * 4.2);
    let halo = exp(-r * r * 1.1) * 0.38;
    let falloff = (core + halo) * edge;
    let warm = vec3f(0.96, 0.94, 0.90);
    let dense = vec3f(0.78, 0.76, 0.72);
    let col = mix(warm, dense, input.opacity * 0.65);
    let alpha = input.opacity * falloff * 0.52 * camera.viewport.w;
    return vec4f(col, alpha);
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
    return vec4f(0.95, 0.62, 0.22, 0.72);
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
    return vec4f(0.28, 0.31, 0.36, 0.55);
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

/// Contrast metrics for preview QA — rejects flat mid-gray frames that still differ from clear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewContrastMetrics {
    pub pixel_count: usize,
    pub max_channel_delta: u8,
    pub mean_luma: f32,
    pub luma_stddev: f32,
    pub pct_pixels_above_delta: f32,
}

const PREVIEW_MIN_MAX_DELTA: u8 = 48;
const PREVIEW_MIN_LUMA_STDDEV: f32 = 8.0;
const PREVIEW_MIN_SMOKE_CENTER_LUMA_STDDEV: f32 = 6.0;
const PREVIEW_MIN_CONTRAST_PIXEL_PCT: f32 = 0.5;
const PREVIEW_MIN_UNIQUE_COLORS: usize = 32;
const PREVIEW_CONTRAST_DELTA_THRESHOLD: u8 = 16;
const PREVIEW_MIN_SMOKE_LUMA: f32 = 52.0;
const PREVIEW_MIN_SMOKE_CENTER_PCT: f32 = 1.2;
const PREVIEW_MIN_SMOKE_BRIGHT_PCT: f32 = 1.5;
const PREVIEW_MAX_SMOKE_BRIGHT_PCT: f32 = 16.0;
const PREVIEW_MIN_SMOKE_BRIGHT_LUMA: f32 = 82.0;
const PREVIEW_MAX_SLAB_BBOX_FILL_RATIO: f32 = 0.88;
const PREVIEW_MAX_SINGLE_BRIGHT_REGION_PCT: f32 = 40.0;
const PREVIEW_MIN_PLUME_MASS_PCT: f32 = 1.0;
const PREVIEW_MIN_SMOKE_BLOB_COUNT: usize = 2;
const PREVIEW_MIN_BLOB_PIXELS: usize = 18;
const PREVIEW_MAX_BILLBOARD_NDC_RADIUS: f32 = 0.16;
const PREVIEW_MAX_BILLBOARD_SCREEN_PX: f32 = 24.0;
const PREVIEW_MIN_BILLBOARD_SCREEN_PX: f32 = 8.0;
const PREVIEW_MAX_BACKGROUND_WASH_PCT: f32 = 82.0;

pub fn preview_contrast_metrics(rgba: &[u8], clear: [u8; 4]) -> PreviewContrastMetrics {
    let pixel_count = rgba.len() / 4;
    if pixel_count == 0 {
        return PreviewContrastMetrics {
            pixel_count: 0,
            max_channel_delta: 0,
            mean_luma: 0.0,
            luma_stddev: 0.0,
            pct_pixels_above_delta: 0.0,
        };
    }

    let mut max_channel_delta = 0u8;
    let mut above_delta = 0usize;
    let mut sum_luma = 0.0f64;
    let mut sum_luma_sq = 0.0f64;

    for px in rgba.chunks_exact(4) {
        let delta = px[0]
            .abs_diff(clear[0])
            .max(px[1].abs_diff(clear[1]))
            .max(px[2].abs_diff(clear[2]));
        max_channel_delta = max_channel_delta.max(delta);
        if delta >= PREVIEW_CONTRAST_DELTA_THRESHOLD {
            above_delta += 1;
        }
        let luma = 0.2126 * px[0] as f64 + 0.7152 * px[1] as f64 + 0.0722 * px[2] as f64;
        sum_luma += luma;
        sum_luma_sq += luma * luma;
    }

    let mean_luma = (sum_luma / pixel_count as f64) as f32;
    let variance = (sum_luma_sq / pixel_count as f64 - sum_luma * sum_luma / (pixel_count as f64 * pixel_count as f64))
        .max(0.0);
    PreviewContrastMetrics {
        pixel_count,
        max_channel_delta,
        mean_luma,
        luma_stddev: variance.sqrt() as f32,
        pct_pixels_above_delta: 100.0 * above_delta as f32 / pixel_count as f32,
    }
}

pub fn preview_has_meaningful_contrast(rgba: &[u8], clear: [u8; 4]) -> bool {
    let m = preview_contrast_metrics(rgba, clear);
    let mut unique = std::collections::HashSet::new();
    for px in rgba.chunks_exact(4) {
        unique.insert((px[0], px[1], px[2]));
    }
    m.max_channel_delta >= PREVIEW_MIN_MAX_DELTA
        && m.luma_stddev >= PREVIEW_MIN_LUMA_STDDEV
        && m.pct_pixels_above_delta >= PREVIEW_MIN_CONTRAST_PIXEL_PCT
        && unique.len() >= PREVIEW_MIN_UNIQUE_COLORS
}

/// Returns true when the center of the frame contains bright smoke-toned pixels (not just grid lines).
pub fn preview_has_smoke_plume(rgba: &[u8], width: u32, height: u32, clear: [u8; 4]) -> bool {
    if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
        return false;
    }
    let cx = width as i32 / 2;
    let cy = height as i32 / 2;
    let rx = (width as i32 / 2).max(48);
    let ry = (height as i32 / 2).max(48);
    let mut center_total = 0usize;
    let mut smoke_pixels = 0usize;
    let mut bright_pixels = 0usize;

    for y in (cy - ry).max(0)..(cy + ry).min(height as i32) {
        for x in (cx - rx).max(0)..(cx + rx).min(width as i32) {
            let i = (y as u32 * width + x as u32) as usize * 4;
            let px = &rgba[i..i + 4];
            let luma =
                0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32;
            let delta = px[0]
                .abs_diff(clear[0])
                .max(px[1].abs_diff(clear[1]))
                .max(px[2].abs_diff(clear[2]));
            center_total += 1;
            if luma >= PREVIEW_MIN_SMOKE_LUMA && delta >= PREVIEW_CONTRAST_DELTA_THRESHOLD {
                smoke_pixels += 1;
            }
            if luma >= PREVIEW_MIN_SMOKE_BRIGHT_LUMA && delta >= PREVIEW_MIN_MAX_DELTA / 2 {
                bright_pixels += 1;
            }
        }
    }

    if center_total == 0 {
        return false;
    }
    let smoke_pct = 100.0 * smoke_pixels as f32 / center_total as f32;
    let bright_pct = 100.0 * bright_pixels as f32 / center_total as f32;
    smoke_pct >= PREVIEW_MIN_SMOKE_CENTER_PCT
        && bright_pct >= PREVIEW_MIN_SMOKE_BRIGHT_PCT
        && bright_pct <= PREVIEW_MAX_SMOKE_BRIGHT_PCT
}

/// Coverage metrics for smoke preview QA.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmokePlumeMetrics {
    pub bright_coverage_pct: f32,
    pub center_bright_pct: f32,
    pub largest_region_pct: f32,
    pub blob_count: usize,
    pub background_wash_pct: f32,
    pub median_screen_radius_px: f32,
}

pub fn preview_smoke_plume_metrics(
    rgba: &[u8],
    width: u32,
    height: u32,
    clear: [u8; 4],
) -> SmokePlumeMetrics {
    let w = width as usize;
    let h = height as usize;
    let pixel_count = w * h;
    if pixel_count == 0 || rgba.len() != pixel_count * 4 {
        return SmokePlumeMetrics {
            bright_coverage_pct: 0.0,
            center_bright_pct: 0.0,
            largest_region_pct: 0.0,
            blob_count: 0,
            background_wash_pct: 0.0,
            median_screen_radius_px: 0.0,
        };
    }

    let cx = w / 2;
    let cy = h / 2;
    let mut bright = 0usize;
    let mut center_bright = 0usize;
    let mut center_total = 0usize;
    let mut color_counts = std::collections::HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let px = &rgba[i..i + 4];
            *color_counts.entry((px[0], px[1], px[2])).or_insert(0usize) += 1;
            if is_bright_smoke_pixel(px, clear) {
                bright += 1;
            }
            if (x as i32 - cx as i32).abs() <= w as i32 / 2
                && (y as i32 - cy as i32).abs() <= h as i32 / 2
            {
                center_total += 1;
                if is_bright_smoke_pixel(px, clear) {
                    center_bright += 1;
                }
            }
        }
    }
    let dominant_pct = color_counts
        .values()
        .max()
        .map(|count| 100.0 * *count as f32 / pixel_count as f32)
        .unwrap_or(0.0);

    SmokePlumeMetrics {
        bright_coverage_pct: 100.0 * bright as f32 / pixel_count as f32,
        center_bright_pct: if center_total > 0 {
            100.0 * center_bright as f32 / center_total as f32
        } else {
            0.0
        },
        largest_region_pct: preview_largest_bright_region_pct(rgba, width, height, clear),
        blob_count: preview_bright_blob_count(rgba, width, height, clear),
        background_wash_pct: dominant_pct,
        median_screen_radius_px: 0.0,
    }
}

fn preview_has_background_wash(rgba: &[u8], clear: [u8; 4]) -> bool {
    let pixel_count = rgba.len() / 4;
    if pixel_count == 0 {
        return false;
    }
    let mut color_counts = std::collections::HashMap::new();
    for px in rgba.chunks_exact(4) {
        *color_counts.entry((px[0], px[1], px[2])).or_insert(0usize) += 1;
    }
    let (dominant, count) = color_counts
        .iter()
        .max_by_key(|(_, c)| *c)
        .map(|(k, v)| (*k, *v))
        .unwrap_or(((clear[0], clear[1], clear[2]), pixel_count));
    let pct = 100.0 * count as f32 / pixel_count as f32;
    if pct < PREVIEW_MAX_BACKGROUND_WASH_PCT {
        return false;
    }
    let luma = 0.2126 * dominant.0 as f32
        + 0.7152 * dominant.1 as f32
        + 0.0722 * dominant.2 as f32;
    let delta = dominant
        .0
        .abs_diff(clear[0])
        .max(dominant.1.abs_diff(clear[1]))
        .max(dominant.2.abs_diff(clear[2]));
    luma > 40.0 && luma < 95.0 && delta >= 12 && delta < PREVIEW_MIN_MAX_DELTA
}

fn is_bright_smoke_pixel(px: &[u8], clear: [u8; 4]) -> bool {
    let luma = 0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32;
    let delta = px[0]
        .abs_diff(clear[0])
        .max(px[1].abs_diff(clear[1]))
        .max(px[2].abs_diff(clear[2]));
    luma >= PREVIEW_MIN_SMOKE_BRIGHT_LUMA && delta >= PREVIEW_CONTRAST_DELTA_THRESHOLD
}

/// Contrast metrics for the center third (ignores grid lines at the periphery).
pub fn preview_center_contrast_metrics(
    rgba: &[u8],
    width: u32,
    height: u32,
    clear: [u8; 4],
) -> PreviewContrastMetrics {
    if width == 0 || height == 0 {
        return preview_contrast_metrics(rgba, clear);
    }
    let cx = width as i32 / 2;
    let cy = height as i32 / 2;
    let rx = (width as i32 / 3).max(32);
    let ry = (height as i32 / 3).max(32);
    let mut samples = Vec::new();
    for y in (cy - ry).max(0)..(cy + ry).min(height as i32) {
        for x in (cx - rx).max(0)..(cx + rx).min(width as i32) {
            let i = (y as u32 * width + x as u32) as usize * 4;
            samples.extend_from_slice(&rgba[i..i + 4]);
        }
    }
    preview_contrast_metrics(&samples, clear)
}

/// Largest 4-connected bright region as a percentage of the frame (rejects flat slabs).
pub fn preview_largest_bright_region_pct(
    rgba: &[u8],
    width: u32,
    height: u32,
    clear: [u8; 4],
) -> f32 {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return 0.0;
    }
    let mut visited = vec![false; w * h];
    let mut largest = 0usize;
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if visited[idx] {
                continue;
            }
            let px = &rgba[idx * 4..idx * 4 + 4];
            if !is_bright_smoke_pixel(px, clear) {
                continue;
            }
            let mut stack = vec![idx];
            visited[idx] = true;
            let mut area = 0usize;
            while let Some(cur) = stack.pop() {
                area += 1;
                let cy = cur / w;
                let cx = cur % w;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let ni = ny as usize * w + nx as usize;
                    if visited[ni] {
                        continue;
                    }
                    let npx = &rgba[ni * 4..ni * 4 + 4];
                    if !is_bright_smoke_pixel(npx, clear) {
                        continue;
                    }
                    visited[ni] = true;
                    stack.push(ni);
                }
            }
            largest = largest.max(area);
        }
    }
    100.0 * largest as f32 / (w * h) as f32
}

/// Counts distinct bright blobs large enough to be smoke puffs (not grid specks).
pub fn preview_bright_blob_count(rgba: &[u8], width: u32, height: u32, clear: [u8; 4]) -> usize {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return 0;
    }
    let mut visited = vec![false; w * h];
    let mut blobs = 0usize;
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if visited[idx] {
                continue;
            }
            let px = &rgba[idx * 4..idx * 4 + 4];
            if !is_bright_smoke_pixel(px, clear) {
                continue;
            }
            let mut stack = vec![idx];
            visited[idx] = true;
            let mut area = 0usize;
            while let Some(cur) = stack.pop() {
                area += 1;
                let cy = cur / w;
                let cx = cur % w;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let ni = ny as usize * w + nx as usize;
                    if visited[ni] {
                        continue;
                    }
                    let npx = &rgba[ni * 4..ni * 4 + 4];
                    if !is_bright_smoke_pixel(npx, clear) {
                        continue;
                    }
                    visited[ni] = true;
                    stack.push(ni);
                }
            }
            if area >= PREVIEW_MIN_BLOB_PIXELS {
                blobs += 1;
            }
        }
    }
    blobs
}

/// Largest bright-region axis-aligned bbox fill ratio (1.0 = solid rectangle / slab).
pub fn preview_bright_bbox_fill_ratio(rgba: &[u8], width: u32, height: u32, clear: [u8; 4]) -> f32 {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return 0.0;
    }
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    let mut area = 0usize;
    for y in 0..h {
        for x in 0..w {
            let px = &rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
            if !is_bright_smoke_pixel(px, clear) {
                continue;
            }
            area += 1;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if area == 0 {
        return 0.0;
    }
    let bw = (max_x - min_x + 1) as f32;
    let bh = (max_y - min_y + 1) as f32;
    area as f32 / (bw * bh).max(1.0)
}

/// Rejects billboard-style clipped slabs (near-rectangular bright silhouettes).
pub fn preview_rejects_sharp_slab_geometry(rgba: &[u8], width: u32, height: u32, clear: [u8; 4]) -> bool {
    let fill = preview_bright_bbox_fill_ratio(rgba, width, height, clear);
    let largest = preview_largest_bright_region_pct(rgba, width, height, clear);
    fill > PREVIEW_MAX_SLAB_BBOX_FILL_RATIO && largest > 8.0
}

/// Rejects empty/sparse/dot frames and flat slabs; accepts soft rising plumes.
pub fn preview_has_soft_smoke_plume(rgba: &[u8], width: u32, height: u32, clear: [u8; 4]) -> bool {
    let metrics = preview_smoke_plume_metrics(rgba, width, height, clear);
    if preview_rejects_sharp_slab_geometry(rgba, width, height, clear) {
        return false;
    }
    if preview_has_background_wash(rgba, clear) {
        return false;
    }
    if !preview_has_smoke_plume(rgba, width, height, clear) {
        return false;
    }
    if metrics.largest_region_pct > PREVIEW_MAX_SINGLE_BRIGHT_REGION_PCT {
        return false;
    }
    if metrics.bright_coverage_pct < PREVIEW_MIN_SMOKE_BRIGHT_PCT
        || metrics.bright_coverage_pct > PREVIEW_MAX_SMOKE_BRIGHT_PCT
    {
        return false;
    }
    if metrics.center_bright_pct < PREVIEW_MIN_SMOKE_BRIGHT_PCT * 0.65 {
        return false;
    }
    let has_plume_mass = metrics.largest_region_pct >= PREVIEW_MIN_PLUME_MASS_PCT;
    let has_multiple_blobs = metrics.blob_count >= PREVIEW_MIN_SMOKE_BLOB_COUNT;
    let connected_plume = metrics.largest_region_pct >= 2.5
        && metrics.largest_region_pct <= 28.0
        && metrics.blob_count >= 1;
    if metrics.largest_region_pct > 28.0 && metrics.blob_count < 2 {
        return false;
    }
    (has_plume_mass && has_multiple_blobs) || connected_plume
}

/// Picks a mid-animation frame where the plume has developed.
pub fn default_smoke_preview_frame(mesh: &ViewportMesh) -> u32 {
    mesh.smoke
        .as_ref()
        .map(|smoke| {
            let count = smoke.frames.len();
            if count <= 1 {
                0
            } else {
                ((count - 1) * 2 / 3).max(1) as u32
            }
        })
        .unwrap_or(0)
}

/// Converts OpenGL clip space (z in [-w, w]) to WebGPU clip space (z in [0, w]).
const OPENGL_TO_WGPU: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 0.5, 0.5],
    [0.0, 0.0, 0.0, 1.0],
];

fn clip_from_view_proj(view: [[f32; 4]; 4], proj: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    mul4(OPENGL_TO_WGPU, mul4(proj, view))
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
    let (mut min, mut max) = bounds_for_mesh(mesh);

    if mesh.positions.is_empty() {
        if let Some(smoke) = &mesh.smoke {
            let frame_idx = default_smoke_preview_frame(mesh) as usize;
            if let Some(frame) = smoke.frames.get(frame_idx) {
                if frame.particle_count > 0 {
                    min = [f32::INFINITY; 3];
                    max = [f32::NEG_INFINITY; 3];
                    let mut mean_size = 0.0f32;
                    for i in 0..frame.particle_count as usize {
                        let p = [
                            frame.positions[i * 3],
                            frame.positions[i * 3 + 1],
                            frame.positions[i * 3 + 2],
                        ];
                        for axis in 0..3 {
                            min[axis] = min[axis].min(p[axis]);
                            max[axis] = max[axis].max(p[axis]);
                        }
                        mean_size += frame.sizes.get(i).copied().unwrap_or(0.25);
                    }
                    mean_size /= frame.particle_count as f32;
                    let pad = (mean_size * 5.5).max(1.2);
                    for axis in 0..3 {
                        min[axis] -= pad;
                        max[axis] += pad;
                    }
                }
            }
        } else if let Some(liquid) = &mesh.liquid {
            if let Some(frame) = liquid.frames.first() {
                if frame.particle_count > 0 {
                    min = [f32::INFINITY; 3];
                    max = [f32::NEG_INFINITY; 3];
                    for i in 0..frame.particle_count as usize {
                        let p = [
                            frame.positions[i * 3],
                            frame.positions[i * 3 + 1],
                            frame.positions[i * 3 + 2],
                        ];
                        for axis in 0..3 {
                            min[axis] = min[axis].min(p[axis]);
                            max[axis] = max[axis].max(p[axis]);
                        }
                    }
                    let pad = 1.2f32;
                    for axis in 0..3 {
                        min[axis] -= pad;
                        max[axis] += pad;
                    }
                }
            }
        }
    }

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
    .fold(1.5_f32, f32::max);
    let smoke_only = mesh.positions.is_empty() && mesh.smoke.is_some();
    let dist_scale = if smoke_only { 1.65 } else { 1.8 };
    let dist = extent * dist_scale;
    NativeViewportCamera {
        eye: [
            center[0] + dist * 0.45,
            center[1] + dist * 0.32,
            center[2] + dist * 1.05,
        ],
        target: [
            center[0],
            center[1] + extent * 0.08,
            center[2],
        ],
        up: [0.0, 1.0, 0.0],
        fov_y_deg: if smoke_only { 34.0 } else { 50.0 },
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
        [s[0], u[0], -f[0], -dot3(s, eye)],
        [s[1], u[1], -f[1], -dot3(u, eye)],
        [s[2], u[2], -f[2], dot3(f, eye)],
        [0.0, 0.0, 0.0, 1.0],
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

/// WGSL `mat4x4 * vec4` — each result row dots the matching matrix row with `v`.
fn mul4_vec4(m: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2] + m[0][3] * v[3],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2] + m[1][3] * v[3],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2] + m[2][3] * v[3],
        m[3][0] * v[0] + m[3][1] * v[1] + m[3][2] * v[2] + m[3][3] * v[3],
    ]
}

fn project_world_to_ndc(
    view_proj: [[f32; 4]; 4],
    world: [f32; 3],
) -> Option<[f32; 3]> {
    let clip = mul4_vec4(view_proj, [world[0], world[1], world[2], 1.0]);
    if clip[3] <= 0.0 {
        return None;
    }
    let inv_w = 1.0 / clip[3];
    Some([clip[0] * inv_w, clip[1] * inv_w, clip[2] * inv_w])
}

fn clip_in_frustum(clip: [f32; 4]) -> bool {
    if clip[3] <= 0.0 {
        return false;
    }
    let w = clip[3];
    // XY overlap is enough for billboard particles; clip Z differs between GL/WGPU paths.
    clip[0].abs() <= w * 1.25 && clip[1].abs() <= w * 1.25
}

#[derive(Debug, Clone, Copy)]
pub struct PreviewProjectionDiagnostics {
    pub particle_count: u32,
    pub position_min: [f32; 3],
    pub position_max: [f32; 3],
    pub particles_in_frustum: u32,
    pub ndc_min: [f32; 3],
    pub ndc_max: [f32; 3],
    pub mean_particle_size: f32,
    pub mean_screen_radius_px: f32,
    pub max_screen_radius_px: f32,
    pub max_ndc_radius: f32,
}

pub fn preview_projection_diagnostics(
    mesh: &ViewportMesh,
    smoke_frame: u32,
    width: u32,
    height: u32,
    camera: &NativeViewportCamera,
) -> PreviewProjectionDiagnostics {
    let view = look_at_rh(camera.eye, camera.target, camera.up);
    let proj = perspective_rh(camera.fov_y_deg, width as f32 / height as f32, 0.1, 500.0);
    let view_proj = clip_from_view_proj(view, proj);
    let tan_half_fov = (0.5 * camera.fov_y_deg.to_radians()).tan();
    let proj_scale = height as f32 / (2.0 * tan_half_fov);

    let mut position_min = [f32::INFINITY; 3];
    let mut position_max = [f32::NEG_INFINITY; 3];
    let mut ndc_min = [f32::INFINITY; 3];
    let mut ndc_max = [f32::NEG_INFINITY; 3];
    let mut particles_in_frustum = 0u32;
    let mut mean_particle_size = 0.0f32;
    let mut mean_screen_radius_px = 0.0f32;
    let mut max_screen_radius_px = 0.0f32;
    let mut max_ndc_radius = 0.0f32;
    let mut particle_count = 0u32;

    if let Some(smoke) = &mesh.smoke {
        if let Some(frame) = smoke.frames.get(smoke_frame as usize) {
            particle_count = frame.particle_count;
            for i in 0..frame.particle_count as usize {
                let p = [
                    frame.positions[i * 3],
                    frame.positions[i * 3 + 1],
                    frame.positions[i * 3 + 2],
                ];
                for axis in 0..3 {
                    position_min[axis] = position_min[axis].min(p[axis]);
                    position_max[axis] = position_max[axis].max(p[axis]);
                }
                let size = frame.sizes.get(i).copied().unwrap_or(0.25);
                let opacity = frame.opacities.get(i).copied().unwrap_or(0.5);
                mean_particle_size += size;
                let to_eye = [
                    camera.eye[0] - p[0],
                    camera.eye[1] - p[1],
                    camera.eye[2] - p[2],
                ];
                let dist = (to_eye[0] * to_eye[0] + to_eye[1] * to_eye[1] + to_eye[2] * to_eye[2])
                    .sqrt()
                    .max(0.35);
                let puff_scale = 0.82 + 0.38 * opacity;
                let world_radius = size * puff_scale;
                let projected_px = world_radius / dist * proj_scale;
                let screen_radius = projected_px.min(PREVIEW_MAX_BILLBOARD_SCREEN_PX);
                mean_screen_radius_px += screen_radius;
                max_screen_radius_px = max_screen_radius_px.max(screen_radius);
                let ndc_radius = screen_radius / (height as f32 * 0.5);
                max_ndc_radius = max_ndc_radius.max(ndc_radius);
                let clip = mul4_vec4(view_proj, [p[0], p[1], p[2], 1.0]);
                if clip[3] > 0.0 {
                    let inv_w = 1.0 / clip[3];
                    let ndc = [clip[0] * inv_w, clip[1] * inv_w, clip[2] * inv_w];
                    for axis in 0..3 {
                        ndc_min[axis] = ndc_min[axis].min(ndc[axis]);
                        ndc_max[axis] = ndc_max[axis].max(ndc[axis]);
                    }
                    if clip_in_frustum(clip) {
                        particles_in_frustum += 1;
                    }
                }
            }
        }
    }

    if particle_count > 0 {
        let n = particle_count as f32;
        mean_particle_size /= n;
        mean_screen_radius_px /= n;
    } else if !position_min[0].is_finite() {
        position_min = [0.0; 3];
        position_max = [0.0; 3];
        ndc_min = [0.0; 3];
        ndc_max = [0.0; 3];
    }

    PreviewProjectionDiagnostics {
        particle_count,
        position_min,
        position_max,
        particles_in_frustum,
        ndc_min,
        ndc_max,
        mean_particle_size,
        mean_screen_radius_px,
        max_screen_radius_px,
        max_ndc_radius,
    }
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

fn sort_smoke_particles_back_to_front(
    particles: &mut [SmokeParticle],
    eye: [f32; 3],
) {
    particles.sort_by(|a, b| {
        let da = (a.center[0] - eye[0]).powi(2)
            + (a.center[1] - eye[1]).powi(2)
            + (a.center[2] - eye[2]).powi(2);
        let db = (b.center[0] - eye[0]).powi(2)
            + (b.center[1] - eye[1]).powi(2)
            + (b.center[2] - eye[2]).powi(2);
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Sparse presets (sphere obstacle) need fewer, tighter puffs to avoid a full-frame slab.
fn smoke_preview_alpha_scale(raw_particle_count: usize) -> f32 {
    if raw_particle_count < 400 {
        0.46
    } else {
        0.96
    }
}

fn smoke_preview_cull_budget(raw_particle_count: usize) -> usize {
    if raw_particle_count < 400 {
        72
    } else {
        680
    }
}

fn smoke_preview_max_screen_px(raw_particle_count: usize) -> f32 {
    if raw_particle_count < 400 {
        14.0
    } else {
        PREVIEW_MAX_BILLBOARD_SCREEN_PX
    }
}

fn smoke_preview_min_opacity(raw_particle_count: usize) -> f32 {
    if raw_particle_count < 400 {
        0.58
    } else {
        0.0
    }
}

/// Keeps the densest impostors for preview rendering to avoid full-frame gray wash.
fn cull_smoke_particles_for_preview(
    particles: &mut Vec<SmokeParticle>,
    max_count: usize,
    min_opacity: f32,
) {
    if min_opacity > 0.0 {
        particles.retain(|p| p.opacity >= min_opacity);
    }
    if particles.len() <= max_count {
        return;
    }
    particles.sort_by(|a, b| {
        let score_a = a.opacity * a.size;
        let score_b = b.opacity * b.size;
        score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
    });
    particles.truncate(max_count);
}

/// Caps impostor sizes so puffs stay readable without merging into a full-frame slab.
fn scale_smoke_sizes_for_screen(
    particles: &mut [SmokeParticle],
    eye: [f32; 3],
    proj_scale: f32,
    max_screen_px: f32,
) {
    for particle in particles.iter_mut() {
        let to = [
            particle.center[0] - eye[0],
            particle.center[1] - eye[1],
            particle.center[2] - eye[2],
        ];
        let dist = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2])
            .sqrt()
            .max(0.35);
        let puff_scale = 0.82 + 0.38 * particle.opacity;
        let projected_px = particle.size * puff_scale / dist * proj_scale;
        if projected_px > max_screen_px {
            particle.size *= max_screen_px / projected_px;
        }
    }
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
    let view_proj = clip_from_view_proj(view, proj);

    let mesh_camera = CameraUniform {
        view_proj,
        light_dir: [0.45, 0.85, 0.25, 0.0],
        eye: [camera.eye[0], camera.eye[1], camera.eye[2], 1.0],
    };

    let tan_half_fov = (0.5 * camera.fov_y_deg.to_radians()).tan();
    let proj_scale = h as f32 / (2.0 * tan_half_fov);
    let raw_smoke_count = mesh
        .smoke
        .as_ref()
        .and_then(|s| s.frames.get(smoke_frame as usize))
        .map(|frame| frame.particle_count as usize)
        .unwrap_or(0);
    let smoke_alpha_scale = smoke_preview_alpha_scale(raw_smoke_count);
    let smoke_camera = SmokeCameraUniform {
        view_proj,
        eye: [camera.eye[0], camera.eye[1], camera.eye[2], 1.0],
        viewport: [w as f32, h as f32, proj_scale, smoke_alpha_scale],
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
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 1.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let grid_overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("grid overlay pipeline"),
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
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
            depth_compare: wgpu::CompareFunction::Always,
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
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 1.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let collider_overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("collider overlay pipeline"),
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
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
            depth_compare: wgpu::CompareFunction::Always,
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
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
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
        .map(|frame| {
            let mut particles = pack_particles(frame);
            let raw_count = frame.particle_count as usize;
            cull_smoke_particles_for_preview(
                &mut particles,
                smoke_preview_cull_budget(raw_count),
                smoke_preview_min_opacity(raw_count),
            );
            scale_smoke_sizes_for_screen(
                &mut particles,
                camera.eye,
                proj_scale,
                smoke_preview_max_screen_px(raw_count),
            );
            sort_smoke_particles_back_to_front(&mut particles, camera.eye);
            particles
        });

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

        let use_density_preview = mesh
            .smoke
            .as_ref()
            .and_then(|s| s.density_frames.get(smoke_frame as usize))
            .map(|d| !d.is_empty())
            .unwrap_or(false);
        if !use_density_preview {
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

        if let (Some(grid_vb), Some(grid_bg)) = (&grid_vertex_buffer, Some(&grid_bind_group)) {
            pass.set_pipeline(&grid_overlay_pipeline);
            pass.set_bind_group(0, grid_bg, &[]);
            pass.set_vertex_buffer(0, grid_vb.slice(..));
            pass.draw(0..grid_lines.len() as u32, 0..1);
        }

        if let (Some(collider_vb), Some(collider_bg)) =
            (&collider_vertex_buffer, Some(&grid_bind_group))
        {
            pass.set_pipeline(&collider_overlay_pipeline);
            pass.set_bind_group(0, collider_bg, &[]);
            pass.set_vertex_buffer(0, collider_vb.slice(..));
            pass.draw(0..collider_lines.len() as u32, 0..1);
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

    if let Some(smoke) = &mesh.smoke {
        if let Some(density) = smoke.density_frames.get(smoke_frame as usize) {
            if !density.is_empty() && smoke.resolution.iter().all(|&r| r > 0) {
                let sparse_preset = smoke
                    .frames
                    .get(smoke_frame as usize)
                    .map(|f| f.particle_count < 400)
                    .unwrap_or(false);
                smoke_density::composite_smoke_density_projection(
                    &mut rgba,
                    w,
                    h,
                    GPU_CLEAR_RGBA,
                    density,
                    smoke.resolution,
                    smoke.bounds_min,
                    smoke.bounds_max,
                    camera,
                    sparse_preset,
                );
            }
        }
    }

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
    fn preview_contrast_metrics_reject_flat_gray() {
        let flat = vec![128u8; 320 * 240 * 4];
        let m = preview_contrast_metrics(&flat, CLEAR_RGBA);
        assert!(m.max_channel_delta >= PREVIEW_MIN_MAX_DELTA);
        assert!(!preview_has_meaningful_contrast(&flat, CLEAR_RGBA));
    }

    #[test]
    fn look_at_maps_target_in_front_of_camera() {
        let eye = [0.0, 0.0, 5.0];
        let target = [0.0, 0.0, 0.0];
        let up = [0.0, 1.0, 0.0];
        let view = look_at_rh(eye, target, up);
        let view_target = mul4_vec4(view, [target[0], target[1], target[2], 1.0]);
        assert!(
            view_target[2] < -1.0 && view_target[2] > -10.0,
            "target should be on -Z in view space, got {view_target:?}"
        );
        let proj = perspective_rh(50.0, 1.0, 0.1, 100.0);
        let clip = mul4_vec4(clip_from_view_proj(view, proj), [target[0], target[1], target[2], 1.0]);
        assert!(clip[3] > 0.0, "target should be in front of camera, clip={clip:?}");
        let ndc = [
            clip[0] / clip[3],
            clip[1] / clip[3],
            clip[2] / clip[3],
        ];
        assert!(
            ndc[0].abs() < 0.01 && ndc[1].abs() < 0.01,
            "target ndc xy should be centered, got {ndc:?}, clip={clip:?}"
        );
    }

    #[test]
    fn smoke_preview_frame_density() {
        for (name, preset) in [
            ("puff", Graph::smoke_puff_preset()),
            ("sphere", Graph::smoke_sphere_preset()),
        ] {
            let mesh = cook_viewport_mesh(&preset).expect("cook");
            let frame = default_smoke_preview_frame(&mesh);
            let camera = default_camera_for_mesh(&mesh);
            let diag = preview_projection_diagnostics(&mesh, frame, 640, 480, &camera);
            eprintln!(
                "{name} frame={frame} particles={} in_frustum={} mean_px={:.1} max_px={:.1} eye={:?} target={:?}",
                diag.particle_count,
                diag.particles_in_frustum,
                diag.mean_screen_radius_px,
                diag.max_screen_radius_px,
                camera.eye,
                camera.target
            );
        }
    }

    #[test]
    fn smoke_particles_project_into_view() {
        let mesh = cook_viewport_mesh(&Graph::smoke_puff_preset()).expect("cook");
        let camera = default_camera_for_mesh(&mesh);
        let frame = default_smoke_preview_frame(&mesh);
        let diag = preview_projection_diagnostics(&mesh, frame, 640, 480, &camera);
        assert!(diag.particle_count > 0, "smoke_puff should cook particles");
        assert!(
            diag.particles_in_frustum > 0,
            "expected particles in frustum, got {diag:?}"
        );
        assert!(
            diag.mean_screen_radius_px >= PREVIEW_MIN_BILLBOARD_SCREEN_PX,
            "particles should cover multiple pixels, got {diag:?}"
        );
        assert!(
            diag.mean_screen_radius_px <= PREVIEW_MAX_BILLBOARD_SCREEN_PX,
            "mean billboard screen radius too large, got {diag:?}"
        );
        assert!(
            diag.max_screen_radius_px <= PREVIEW_MAX_BILLBOARD_SCREEN_PX,
            "billboard screen radius too large, got {diag:?}"
        );
        assert!(
            diag.max_ndc_radius <= PREVIEW_MAX_BILLBOARD_NDC_RADIUS,
            "billboard NDC radius too large, got {diag:?}"
        );
    }

    #[test]
    fn density_projection_paints_visible_cloud() {
        let mesh = cook_viewport_mesh(&Graph::smoke_puff_preset()).expect("cook");
        let smoke = mesh.smoke.as_ref().expect("smoke");
        let frame = default_smoke_preview_frame(&mesh) as usize;
        let density = &smoke.density_frames[frame];
        let camera = default_camera_for_mesh(&mesh);
        let w = 160u32;
        let h = 120u32;
        let mut rgba = vec![0u8; w as usize * h as usize * 4];
        for i in (0..rgba.len()).step_by(4) {
            rgba[i..i + 4].copy_from_slice(&GPU_CLEAR_RGBA);
        }
        smoke_density::composite_smoke_density_projection(
            &mut rgba,
            w,
            h,
            GPU_CLEAR_RGBA,
            density,
            smoke.resolution,
            smoke.bounds_min,
            smoke.bounds_max,
            &camera,
            smoke.frames.first().map(|f| f.particle_count < 400).unwrap_or(false),
        );
        let painted = rgba
            .chunks_exact(4)
            .filter(|px| px != &GPU_CLEAR_RGBA)
            .count();
        let painted_pct = 100.0 * painted as f32 / (w * h) as f32;
        assert!(
            painted_pct > 1.5 && painted_pct < 45.0,
            "expected soft cloud coverage, got {painted_pct}%"
        );
    }

    #[test]
    fn smoke_viewport_carries_density_frames() {
        let mesh = cook_viewport_mesh(&Graph::smoke_puff_preset()).expect("cook");
        let smoke = mesh.smoke.as_ref().expect("smoke");
        assert!(!smoke.density_frames.is_empty(), "expected density grids");
        assert!(smoke.resolution[0] > 0);
        assert_eq!(smoke.density_frames.len(), smoke.frames.len());
        let frame = default_smoke_preview_frame(&mesh) as usize;
        let grid = &smoke.density_frames[frame];
        let max_d = grid.iter().copied().fold(0.0_f32, f32::max);
        assert!(max_d > 0.0, "density frame should be non-empty");
    }

    #[test]
    fn render_smoke_puff_has_meaningful_contrast() {
        let mesh = cook_viewport_mesh(&Graph::smoke_puff_preset()).expect("cook");
        let camera = default_camera_for_mesh(&mesh);
        let frame = default_smoke_preview_frame(&mesh);
        let preview = render_native_viewport(&mesh, frame, 640, 480, &camera).expect("render");
        let rgba = preview.decode_rgba().expect("decode");
        assert_eq!(rgba.len(), 640 * 480 * 4);
        let plume = preview_smoke_plume_metrics(&rgba, preview.width, preview.height, GPU_CLEAR_RGBA);
        assert!(
            preview_has_soft_smoke_plume(&rgba, preview.width, preview.height, GPU_CLEAR_RGBA),
            "smoke_puff plume metrics: {:?}",
            plume
        );
    }

    #[test]
    fn render_smoke_sphere_has_meaningful_contrast() {
        let mesh = cook_viewport_mesh(&Graph::smoke_sphere_preset()).expect("cook");
        let camera = default_camera_for_mesh(&mesh);
        let frame = default_smoke_preview_frame(&mesh);
        let preview = render_native_viewport(&mesh, frame, 640, 480, &camera).expect("render");
        let rgba = preview.decode_rgba().expect("decode");
        let plume = preview_smoke_plume_metrics(&rgba, preview.width, preview.height, GPU_CLEAR_RGBA);
        assert!(
            preview_has_soft_smoke_plume(&rgba, preview.width, preview.height, GPU_CLEAR_RGBA),
            "smoke_sphere plume metrics: {:?}",
            plume
        );
    }

    #[test]
    fn render_smoke_puff_writes_preview_artifact() {
        let mesh = cook_viewport_mesh(&Graph::smoke_puff_preset()).expect("cook");
        let camera = default_camera_for_mesh(&mesh);
        let frame = default_smoke_preview_frame(&mesh);
        let preview = render_native_viewport(&mesh, frame, 640, 480, &camera).expect("render");
        let rgba = preview.decode_rgba().expect("decode");
        let artifact_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/tests/artifacts");
        std::fs::create_dir_all(&artifact_dir).expect("artifact dir");
        let png_path = artifact_dir.join("smoke_puff_wgpu_preview.png");
        write_rgba_png(&png_path, preview.width, preview.height, &rgba).expect("write png");

        let plume = preview_smoke_plume_metrics(&rgba, preview.width, preview.height, GPU_CLEAR_RGBA);
        assert!(
            preview_has_soft_smoke_plume(&rgba, preview.width, preview.height, GPU_CLEAR_RGBA),
            "smoke_puff plume: {:?}",
            plume
        );
        assert!(png_path.exists());
        assert!(std::fs::metadata(&png_path).expect("png metadata").len() > 1024);
    }

    #[test]
    fn render_smoke_sphere_writes_preview_artifact() {
        let mesh = cook_viewport_mesh(&Graph::smoke_sphere_preset()).expect("cook");
        let camera = default_camera_for_mesh(&mesh);
        let frame = default_smoke_preview_frame(&mesh);
        let preview = render_native_viewport(&mesh, frame, 640, 480, &camera).expect("render");
        let rgba = preview.decode_rgba().expect("decode");

        let artifact_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/tests/artifacts");
        std::fs::create_dir_all(&artifact_dir).expect("artifact dir");
        let png_path = artifact_dir.join("smoke_sphere_wgpu_preview.png");
        write_rgba_png(&png_path, preview.width, preview.height, &rgba).expect("write png");

        let plume = preview_smoke_plume_metrics(&rgba, preview.width, preview.height, GPU_CLEAR_RGBA);
        assert!(
            preview_has_soft_smoke_plume(&rgba, preview.width, preview.height, GPU_CLEAR_RGBA),
            "smoke_sphere plume: {:?}",
            plume
        );
        assert!(png_path.exists());
        assert!(std::fs::metadata(&png_path).expect("png metadata").len() > 1024);
    }

    #[test]
    fn render_shop_street_has_meaningful_contrast() {
        let mesh = cook_viewport_mesh(&Graph::shop_street_preset()).expect("cook");
        let camera = default_camera_for_mesh(&mesh);
        let preview = render_native_viewport(&mesh, 0, 640, 480, &camera).expect("render");
        let rgba = preview.decode_rgba().expect("decode");
        let center_metrics =
            preview_center_contrast_metrics(&rgba, preview.width, preview.height, CLEAR_RGBA);
        assert!(
            preview_has_meaningful_contrast(&rgba, CLEAR_RGBA)
                || center_metrics.max_channel_delta >= PREVIEW_MIN_MAX_DELTA,
            "shop_street preview metrics: {:?} center: {:?}",
            preview_contrast_metrics(&rgba, CLEAR_RGBA),
            center_metrics
        );
    }

    #[test]
    fn render_shop_street_writes_preview_artifact() {
        let mesh = cook_viewport_mesh(&Graph::shop_street_preset()).expect("cook");
        let camera = default_camera_for_mesh(&mesh);
        let preview = render_native_viewport(&mesh, 0, 640, 480, &camera).expect("render");
        let rgba = preview.decode_rgba().expect("decode");
        let center_metrics =
            preview_center_contrast_metrics(&rgba, preview.width, preview.height, CLEAR_RGBA);
        assert!(
            preview_has_meaningful_contrast(&rgba, CLEAR_RGBA)
                || center_metrics.max_channel_delta >= PREVIEW_MIN_MAX_DELTA
        );

        let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/tests/artifacts");
        std::fs::create_dir_all(&artifact_dir).expect("artifact dir");
        let png_path = artifact_dir.join("shop_street_wgpu_preview.png");
        write_rgba_png(&png_path, preview.width, preview.height, &rgba).expect("write png");
        assert!(png_path.exists());
        assert!(std::fs::metadata(&png_path).expect("png metadata").len() > 1024);
    }


    #[test]
    fn slab_heuristics_reject_single_flat_region() {
        let w = 64u32;
        let h = 48u32;
        let mut rgba = vec![CLEAR_RGBA[0], CLEAR_RGBA[1], CLEAR_RGBA[2], CLEAR_RGBA[3]];
        rgba.resize(w as usize * h as usize * 4, 0);
        for y in 0..h {
            for x in 0..w / 2 {
                let i = (y as usize * w as usize + x as usize) * 4;
                rgba[i] = 220;
                rgba[i + 1] = 215;
                rgba[i + 2] = 205;
                rgba[i + 3] = 255;
            }
        }
        let largest = preview_largest_bright_region_pct(&rgba, w, h, CLEAR_RGBA);
        assert!(
            largest > PREVIEW_MAX_SINGLE_BRIGHT_REGION_PCT,
            "flat slab should exceed region cap, got {largest}%"
        );
        assert!(
            !preview_has_soft_smoke_plume(&rgba, w, h, CLEAR_RGBA),
            "single slab should not pass soft plume QA"
        );
    }

    #[test]
    fn slab_heuristics_accept_multiple_blobs() {
        let w = 96u32;
        let h = 72u32;
        let mut rgba = vec![CLEAR_RGBA[0], CLEAR_RGBA[1], CLEAR_RGBA[2], CLEAR_RGBA[3]];
        rgba.resize(w as usize * h as usize * 4, 0);
        let centers = [(24, 20), (48, 30), (70, 18), (36, 50), (60, 55), (18, 42)];
        for &(cx, cy) in &centers {
            for y in 0..h {
                for x in 0..w {
                    let dx = x as i32 - cx;
                    let dy = y as i32 - cy;
                    if (dx * dx + dy * dy) > 42 {
                        continue;
                    }
                    let i = (y as usize * w as usize + x as usize) * 4;
                    rgba[i] = 210;
                    rgba[i + 1] = 205;
                    rgba[i + 2] = 198;
                    rgba[i + 3] = 255;
                }
            }
        }
        let blobs = preview_bright_blob_count(&rgba, w, h, CLEAR_RGBA);
        assert!(
            blobs >= PREVIEW_MIN_SMOKE_BLOB_COUNT,
            "expected multiple blobs, got {blobs}"
        );
        let largest = preview_largest_bright_region_pct(&rgba, w, h, CLEAR_RGBA);
        assert!(
            preview_has_soft_smoke_plume(&rgba, w, h, CLEAR_RGBA),
            "multi-blob plume should pass soft plume QA (largest={largest}%)"
        );
    }

    fn write_rgba_png(path: &std::path::Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
        use std::io::Write;
        let file = std::fs::File::create(path).map_err(|err| err.to_string())?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|err| err.to_string())?;
        writer.write_image_data(rgba).map_err(|err| err.to_string())?;
        writer.finish().map_err(|err| err.to_string())?;
        Ok(())
    }
}

