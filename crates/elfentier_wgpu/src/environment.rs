//! HDR equirectangular environment maps for viewport background + volume IBL.

use elfentier_core::environment::{ViewportEnvironment, ViewportEnvironmentPreset};
use std::f32::consts::PI;
use std::path::Path;

pub const ENV_MAP_WIDTH: u32 = 256;
pub const ENV_MAP_HEIGHT: u32 = 128;

/// CPU-side RGB radiance map (linear, HDR values allowed).
#[derive(Debug, Clone)]
pub struct EnvironmentMap {
    pub width: u32,
    pub height: u32,
    /// RGB linear radiance, row-major `height * width * 3`.
    pub rgb: Vec<f32>,
}

impl EnvironmentMap {
    pub fn pixel(&self, x: u32, y: u32) -> [f32; 3] {
        let idx = (y * self.width + x) as usize * 3;
        [
            self.rgb[idx],
            self.rgb[idx + 1],
            self.rgb[idx + 2],
        ]
    }
}

/// Builds an environment map from settings (preset or file).
pub fn build_environment_map(env: &ViewportEnvironment) -> Result<EnvironmentMap, String> {
    match env.effective_preset() {
        ViewportEnvironmentPreset::FlatGray => Ok(flat_gray_map()),
        ViewportEnvironmentPreset::StudioSoft => Ok(generate_studio_soft(ENV_MAP_WIDTH, ENV_MAP_HEIGHT)),
        ViewportEnvironmentPreset::StudioContrast => {
            Ok(generate_studio_contrast(ENV_MAP_WIDTH, ENV_MAP_HEIGHT))
        }
        ViewportEnvironmentPreset::Custom => {
            let path = env
                .hdr_path
                .as_deref()
                .filter(|p| !p.is_empty())
                .ok_or("custom environment requires hdr_path")?;
            load_environment_from_path(path)
        }
    }
}

fn flat_gray_map() -> EnvironmentMap {
    let w = 4u32;
    let h = 2u32;
    let gray = [0.047, 0.055, 0.071];
    EnvironmentMap {
        width: w,
        height: h,
        rgb: gray
            .iter()
            .cycle()
            .take((w * h * 3) as usize)
            .copied()
            .collect(),
    }
}

/// Soft studio: warm floor gradient + diffuse overhead key.
pub fn generate_studio_soft(width: u32, height: u32) -> EnvironmentMap {
    let mut rgb = vec![0.0f32; width as usize * height as usize * 3];
    for y in 0..height {
        for x in 0..width {
            let u = (x as f32 + 0.5) / width as f32;
            let v = (y as f32 + 0.5) / height as f32;
            let theta = u * 2.0 * PI;
            let phi = v * PI;
            let dir = direction_from_latlong(theta, phi);
            let floor = smoothstep(0.0, 0.35, -dir[1]);
            let sky = smoothstep(0.15, 0.95, dir[1]);
            let key_u = 0.28;
            let key_v = 0.22;
            let key_dist = ((u - key_u).powi(2) + (v - key_v).powi(2)).sqrt();
            let key = exp(-key_dist * key_dist / (2.0 * 0.08 * 0.08)) * 2.4;
            let fill = exp(-key_dist * key_dist / (2.0 * 0.22 * 0.22)) * 0.55;
            let base = [
                0.12 + 0.08 * floor + 0.18 * sky,
                0.13 + 0.07 * floor + 0.20 * sky,
                0.16 + 0.06 * floor + 0.24 * sky,
            ];
            let warm_key = [1.05, 0.98, 0.88];
            let col = [
                base[0] + warm_key[0] * (key + fill),
                base[1] + warm_key[1] * (key + fill),
                base[2] + warm_key[2] * (key + fill),
            ];
            let idx = (y * width + x) as usize * 3;
            rgb[idx] = col[0];
            rgb[idx + 1] = col[1];
            rgb[idx + 2] = col[2];
        }
    }
    EnvironmentMap {
        width,
        height,
        rgb,
    }
}

/// Higher-contrast studio: darker sides, bright overhead strip.
pub fn generate_studio_contrast(width: u32, height: u32) -> EnvironmentMap {
    let mut rgb = vec![0.0f32; width as usize * height as usize * 3];
    for y in 0..height {
        for x in 0..width {
            let u = (x as f32 + 0.5) / width as f32;
            let v = (y as f32 + 0.5) / height as f32;
            let theta = u * 2.0 * PI;
            let phi = v * PI;
            let dir = direction_from_latlong(theta, phi);
            let overhead = exp(-((v - 0.12).powi(2)) / (2.0 * 0.04 * 0.04)) * 3.2;
            let side_dark = 0.35 + 0.65 * dir[1].abs();
            let rim = smoothstep(0.55, 1.0, dir[1].abs()) * 0.25;
            let col = [
                (0.06 + overhead * 1.02 + rim) / side_dark,
                (0.07 + overhead * 1.0 + rim * 0.9) / side_dark,
                (0.10 + overhead * 0.95 + rim * 0.8) / side_dark,
            ];
            let idx = (y * width + x) as usize * 3;
            rgb[idx] = col[0];
            rgb[idx + 1] = col[1];
            rgb[idx + 2] = col[2];
        }
    }
    EnvironmentMap {
        width,
        height,
        rgb,
    }
}

/// Loads an equirectangular HDR/EXR/LDR image from disk.
pub fn load_environment_from_path(path: impl AsRef<Path>) -> Result<EnvironmentMap, String> {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "hdr" => load_radiance_hdr(path),
        "exr" => load_exr(path),
        "png" | "jpg" | "jpeg" => load_ldr_image(path),
        other => Err(format!(
            "unsupported environment image extension '.{other}' (use .hdr, .exr, .png, .jpg)"
        )),
    }
}

fn load_radiance_hdr(path: &Path) -> Result<EnvironmentMap, String> {
    let img = image::open(path)
        .map_err(|err| format!("failed to decode HDR '{}': {err}", path.display()))?
        .into_rgb32f();
    let width = img.width();
    let height = img.height();
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for px in img.pixels() {
        rgb.push(px[0]);
        rgb.push(px[1]);
        rgb.push(px[2]);
    }
    Ok(EnvironmentMap {
        width,
        height,
        rgb,
    })
}

fn load_exr(path: &Path) -> Result<EnvironmentMap, String> {
    use exr::prelude::*;
    use image::{ImageBuffer, Rgb};
    type Rgb32fImage = ImageBuffer<Rgb<f32>, Vec<f32>>;
    let image = read_first_rgba_layer_from_file(
        path,
        |resolution, _channels: &RgbaChannels| -> Rgb32fImage {
            ImageBuffer::new(resolution.width() as u32, resolution.height() as u32)
        },
        |pixels: &mut Rgb32fImage,
         position: Vec2<usize>,
         (r, g, b, _a): (f32, f32, f32, f32)| {
            pixels.put_pixel(position.x() as u32, position.y() as u32, Rgb([r, g, b]));
        },
    )
    .map_err(|err| format!("failed to decode EXR '{}': {err}", path.display()))?;
    let pixels = image.layer_data.channel_data.pixels;
    let width = pixels.width();
    let height = pixels.height();
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for px in pixels.pixels() {
        rgb.push(px[0]);
        rgb.push(px[1]);
        rgb.push(px[2]);
    }
    Ok(EnvironmentMap {
        width,
        height,
        rgb,
    })
}

fn load_ldr_image(path: &Path) -> Result<EnvironmentMap, String> {
    let img = image::open(path)
        .map_err(|err| format!("failed to open image '{}': {err}", path.display()))?
        .into_rgb8();
    let width = img.width();
    let height = img.height();
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for px in img.pixels() {
        rgb.push(px[0] as f32 / 255.0);
        rgb.push(px[1] as f32 / 255.0);
        rgb.push(px[2] as f32 / 255.0);
    }
    Ok(EnvironmentMap {
        width,
        height,
        rgb,
    })
}

/// Packs RGB f32 map into RGBA16Float bytes for GPU upload (filterable HDR).
pub fn pack_rgba16f(map: &EnvironmentMap) -> Vec<u8> {
    let mut out = Vec::with_capacity(map.width as usize * map.height as usize * 8);
    for px in map.rgb.chunks_exact(3) {
        out.extend_from_slice(&half::f16::from_f32(px[0]).to_bits().to_le_bytes());
        out.extend_from_slice(&half::f16::from_f32(px[1]).to_bits().to_le_bytes());
        out.extend_from_slice(&half::f16::from_f32(px[2]).to_bits().to_le_bytes());
        out.extend_from_slice(&half::f16::from_f32(1.0).to_bits().to_le_bytes());
    }
    out
}

/// Lat-long equirectangular sample (CPU reference for tests).
pub fn sample_equirect(
    map: &EnvironmentMap,
    direction: [f32; 3],
    rotation_yaw_rad: f32,
    intensity: f32,
    diffuse_blur: f32,
) -> [f32; 3] {
    let dir = rotate_y(normalize3(direction), rotation_yaw_rad);
    if diffuse_blur <= 0.001 {
        return scale3(sample_equirect_sharp(map, dir), intensity);
    }
    let taps = 5usize;
    let spread = diffuse_blur * 0.18;
    let mut acc = [0.0f32; 3];
    for i in 0..taps {
        let angle = (i as f32 / taps as f32) * 2.0 * PI;
        let offset = [
            spread * angle.cos(),
            spread * 0.5 * angle.sin(),
            spread * 0.35 * (angle * 1.7).cos(),
        ];
        let tap_dir = normalize3([
            dir[0] + offset[0],
            dir[1] + offset[1],
            dir[2] + offset[2],
        ]);
        let s = sample_equirect_sharp(map, tap_dir);
        acc[0] += s[0];
        acc[1] += s[1];
        acc[2] += s[2];
    }
    scale3(
        [acc[0] / taps as f32, acc[1] / taps as f32, acc[2] / taps as f32],
        intensity,
    )
}

fn sample_equirect_sharp(map: &EnvironmentMap, dir: [f32; 3]) -> [f32; 3] {
    let theta = dir[0].atan2(dir[2]);
    let phi = dir[1].clamp(-1.0, 1.0).asin();
    let u = (theta / (2.0 * PI)) + 0.5;
    let v = 0.5 - phi / PI;
    let x = (u * map.width as f32).clamp(0.0, map.width as f32 - 1.0);
    let y = (v * map.height as f32).clamp(0.0, map.height as f32 - 1.0);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(map.width - 1);
    let y1 = (y0 + 1).min(map.height - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let c00 = map.pixel(x0, y0);
    let c10 = map.pixel(x1, y0);
    let c01 = map.pixel(x0, y1);
    let c11 = map.pixel(x1, y1);
    lerp3(lerp3(c00, c10, tx), lerp3(c01, c11, tx), ty)
}

pub fn direction_from_latlong(theta: f32, phi: f32) -> [f32; 3] {
    let sin_phi = phi.sin();
    [
        sin_phi * theta.sin(),
        phi.cos(),
        sin_phi * theta.cos(),
    ]
}

fn rotate_y(v: [f32; 3], yaw: f32) -> [f32; 3] {
    let c = yaw.cos();
    let s = yaw.sin();
    [c * v[0] + s * v[2], v[1], -s * v[0] + c * v[2]]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn scale3(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn exp(v: f32) -> f32 {
    v.exp()
}

/// Simple Reinhard tonemap for LDR readback reference.
pub fn tonemap_reinhard(rgb: [f32; 3]) -> [f32; 3] {
    [
        rgb[0] / (1.0 + rgb[0]),
        rgb[1] / (1.0 + rgb[1]),
        rgb[2] / (1.0 + rgb[2]),
    ]
}

/// Returns true when horizontal scan lines differ enough (not flat gray).
pub fn map_has_horizontal_variation(map: &EnvironmentMap) -> bool {
    if map.height < 2 {
        return false;
    }
    let mid = map.height / 2;
    let mut max_delta = 0.0f32;
    for x in 0..map.width {
        let top = map.pixel(x, 0);
        let mid_px = map.pixel(x, mid);
        for c in 0..3 {
            max_delta = max_delta.max((top[c] - mid_px[c]).abs());
        }
    }
    max_delta > 0.04
}

pub const BACKGROUND_SHADER: &str = r#"
struct EnvUniform {
    params: vec4f,
    viewport: vec4f,
    eye: vec4f,
    forward: vec4f,
    right: vec4f,
    up: vec4f,
}

@group(0) @binding(0) var<uniform> env: EnvUniform;
@group(0) @binding(1) var env_tex: texture_2d<f32>;
@group(0) @binding(2) var env_sampler: sampler;

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
    out.clip = vec4f(pos, 1.0, 1.0);
    out.ndc = pos;
    return out;
}

fn rotate_y_vec(v: vec3f, yaw: f32) -> vec3f {
    let c = cos(yaw);
    let s = sin(yaw);
    return vec3f(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
}

fn sample_env_sharp(rd: vec3f) -> vec3f {
    let dir = rotate_y_vec(normalize(rd), env.params.y);
    let theta = atan2(dir.x, dir.z);
    let phi = asin(clamp(dir.y, -1.0, 1.0));
    let u = theta / (2.0 * 3.14159265) + 0.5;
    let v = 0.5 - phi / 3.14159265;
    return textureSampleLevel(env_tex, env_sampler, vec2f(u, v), 0.0).rgb;
}

fn sample_env_diffuse(rd: vec3f) -> vec3f {
    let blur = env.params.z;
    if (blur < 0.001) {
        return sample_env_sharp(rd);
    }
    let spread = blur * 0.18;
    var acc = vec3f(0.0);
    let taps = 5.0;
    for (var i = 0.0; i < taps; i += 1.0) {
        let angle = (i / taps) * 2.0 * 3.14159265;
        let offset = vec3f(
            spread * cos(angle),
            spread * 0.5 * sin(angle),
            spread * 0.35 * cos(angle * 1.7),
        );
        acc += sample_env_sharp(normalize(rd + offset));
    }
    return acc / taps;
}

fn tonemap(col: vec3f) -> vec3f {
    return col / (vec3f(1.0) + col);
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4f {
    let aspect = env.viewport.x;
    let tan_half = env.viewport.y;
    let rd = normalize(
        env.forward.xyz
            + env.right.xyz * input.ndc.x * tan_half * aspect
            + env.up.xyz * input.ndc.y * tan_half,
    );
    var col = sample_env_diffuse(rd) * env.params.x;
    col = tonemap(col);
    return vec4f(col, 1.0);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EnvUniform {
    pub params: [f32; 4],
    pub viewport: [f32; 4],
    pub eye: [f32; 4],
    pub forward: [f32; 4],
    pub right: [f32; 4],
    pub up: [f32; 4],
}

pub fn build_env_uniform(
    env: &ViewportEnvironment,
    eye: [f32; 3],
    forward: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    aspect: f32,
    tan_half_fov: f32,
) -> EnvUniform {
    EnvUniform {
        params: [
            env.intensity.max(0.0),
            env.rotation_yaw_deg.to_radians(),
            env.diffuse_blur.clamp(0.0, 1.0),
            if env.enabled { 1.0 } else { 0.0 },
        ],
        viewport: [aspect, tan_half_fov, 0.0, 0.0],
        eye: [eye[0], eye[1], eye[2], 1.0],
        forward: [forward[0], forward[1], forward[2], 0.0],
        right: [right[0], right[1], right[2], 0.0],
        up: [up[0], up[1], up[2], 0.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studio_soft_has_horizontal_variation() {
        let map = generate_studio_soft(64, 32);
        assert!(map_has_horizontal_variation(&map));
    }

    #[test]
    fn studio_contrast_brighter_than_floor() {
        let map = generate_studio_contrast(64, 32);
        let floor = sample_equirect(&map, [0.0, -1.0, 0.0], 0.0, 1.0, 0.0);
        let top = sample_equirect(&map, [0.0, 1.0, 0.0], 0.0, 1.0, 0.0);
        assert!(
            top[0] > floor[0] && top[1] > floor[1],
            "zenith should be brighter than nadir: top={top:?} floor={floor:?}"
        );
    }

    #[test]
    fn equirect_sampling_not_uniform_gray() {
        let map = generate_studio_soft(64, 32);
        let left = sample_equirect(&map, [-1.0, 0.2, 0.0], 0.0, 1.0, 0.0);
        let right = sample_equirect(&map, [1.0, 0.2, 0.0], 0.0, 1.0, 0.0);
        let delta = (left[0] - right[0]).abs() + (left[1] - right[1]).abs();
        assert!(delta > 0.05, "expected lateral env variation, delta={delta}");
    }
}
