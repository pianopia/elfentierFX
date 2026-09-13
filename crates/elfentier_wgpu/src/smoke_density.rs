//! CPU density projection for smoke viewport previews (screen-space voxel splat + MIP).

use crate::{GPU_CLEAR_RGBA, NativeViewportCamera};

/// Alpha-composites a soft volumetric smoke cloud into `rgba` behind grid/collider overlays.
pub fn composite_smoke_density_projection(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    clear: [u8; 4],
    density: &[f32],
    resolution: [u32; 3],
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    camera: &NativeViewportCamera,
    sparse_preset: bool,
) {
    let nx = resolution[0].max(1) as usize;
    let ny = resolution[1].max(1) as usize;
    let nz = resolution[2].max(1) as usize;
    if density.len() < nx * ny * nz {
        return;
    }

    let max_density = density.iter().copied().fold(0.0_f32, f32::max).max(1e-5);
    let baseline = density_baseline(density);
    let density_scale = (max_density - baseline).max(1e-5);
    let voxel_thresh = baseline + density_scale * if sparse_preset { 0.08 } else { 0.20 };
    let radius_boost = if sparse_preset { 2.2 } else { 1.0 };

    let w = width as usize;
    let h = height as usize;
    let mut field = vec![0.0_f32; w * h];

    let aspect = width as f32 / height as f32;
    let tan_half = (0.5 * camera.fov_y_deg.to_radians()).tan();
    let (forward, right, up) = camera_basis(camera);
    let vs = [
        (bounds_max[0] - bounds_min[0]) / nx as f32,
        (bounds_max[1] - bounds_min[1]) / ny as f32,
        (bounds_max[2] - bounds_min[2]) / nz as f32,
    ];

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let idx = i + nx * (j + ny * k);
                let raw = density[idx];
                if raw < voxel_thresh {
                    continue;
                }
                let norm = ((raw - baseline) / density_scale).clamp(0.0, 1.0);
                let world = [
                    bounds_min[0] + (i as f32 + 0.5) * vs[0],
                    bounds_min[1] + (j as f32 + 0.5) * vs[1],
                    bounds_min[2] + (k as f32 + 0.5) * vs[2],
                ];
                if let Some((sx, sy, depth)) =
                    project_to_screen(world, camera, forward, right, up, aspect, tan_half, w, h)
                {
                    let radius = radius_boost
                        * (3.0 + norm.powf(0.82) * 8.0)
                        / depth.max(0.6)
                        * (height as f32 * 0.5)
                        / tan_half;
                    splat_gaussian_max(&mut field, w, h, sx, sy, radius, norm);
                }
            }
        }
    }

    let clear_f = [
        GPU_CLEAR_RGBA[0] as f32 / 255.0,
        GPU_CLEAR_RGBA[1] as f32 / 255.0,
        GPU_CLEAR_RGBA[2] as f32 / 255.0,
    ];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let px = &rgba[i..i + 4];
            if is_overlay_pixel(px) {
                continue;
            }
            let v = field[y * w + x];
            let mip = smoothstep(0.10, 0.50, v);
            if mip < 0.14 {
                continue;
            }
            let alpha = (mip * if sparse_preset { 0.98 } else { 0.88 }).clamp(0.12, 0.94);
            let warm = [0.96_f32, 0.94, 0.90];
            let dense = [0.72, 0.70, 0.66];
            let col = [
                warm[0] * (1.0 - mip) + dense[0] * mip,
                warm[1] * (1.0 - mip) + dense[1] * mip,
                warm[2] * (1.0 - mip) + dense[2] * mip,
            ];
            for c in 0..3 {
                let out = col[c] * alpha + clear_f[c] * (1.0 - alpha);
                rgba[i + c] = (out.clamp(0.0, 1.0) * 255.0) as u8;
            }
            rgba[i + 3] = 255;
        }
    }
}

fn project_to_screen(
    world: [f32; 3],
    camera: &NativeViewportCamera,
    forward: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    aspect: f32,
    tan_half: f32,
    w: usize,
    h: usize,
) -> Option<(f32, f32, f32)> {
    let to = [
        world[0] - camera.eye[0],
        world[1] - camera.eye[1],
        world[2] - camera.eye[2],
    ];
    let depth = dot3(to, forward);
    if depth < 0.15 {
        return None;
    }
    let sx = dot3(to, right) / (depth * tan_half * aspect);
    let sy = dot3(to, up) / (depth * tan_half);
    if sx.abs() > 1.35 || sy.abs() > 1.35 {
        return None;
    }
    let px = (sx * 0.5 + 0.5) * w as f32;
    let py = (1.0 - (sy * 0.5 + 0.5)) * h as f32;
    Some((px, py, depth))
}

fn splat_gaussian_max(field: &mut [f32], w: usize, h: usize, cx: f32, cy: f32, radius: f32, strength: f32) {
    let r = radius.clamp(1.5, 9.0);
    let r2 = r * r;
    let x0 = (cx - r).floor().max(0.0) as i32;
    let y0 = (cy - r).floor().max(0.0) as i32;
    let x1 = (cx + r).ceil().min(w as f32 - 1.0) as i32;
    let y1 = (cy + r).ceil().min(h as f32 - 1.0) as i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d2 = dx * dx + dy * dy;
            if d2 > r2 {
                continue;
            }
            let falloff = (-d2 / (r2 * 0.42)).exp();
            let v = strength * falloff;
            let idx = y as usize * w + x as usize;
            if v > field[idx] {
                field[idx] = v;
            }
        }
    }
}

fn density_baseline(density: &[f32]) -> f32 {
    let mut samples: Vec<f32> = density.iter().copied().filter(|v| *v > 0.0).collect();
    if samples.is_empty() {
        return 0.0;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = (samples.len() as f32 * 0.30) as usize;
    samples[idx.min(samples.len() - 1)]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn camera_basis(camera: &NativeViewportCamera) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let forward = normalize3([
        camera.target[0] - camera.eye[0],
        camera.target[1] - camera.eye[1],
        camera.target[2] - camera.eye[2],
    ]);
    let right = normalize3(cross3(camera.up, forward));
    let up = cross3(forward, right);
    (forward, right, up)
}

fn is_overlay_pixel(px: &[u8]) -> bool {
    if is_gpu_clear_pixel(px) {
        return false;
    }
    if px[0] > 180 && px[1] > 110 && px[2] < 120 {
        return true;
    }
    let delta = px[0]
        .abs_diff(GPU_CLEAR_RGBA[0])
        .max(px[1].abs_diff(GPU_CLEAR_RGBA[1]))
        .max(px[2].abs_diff(GPU_CLEAR_RGBA[2]));
    if delta < 90 && px[0] < 120 && px[1] < 120 {
        return true;
    }
    false
}

fn is_gpu_clear_pixel(px: &[u8]) -> bool {
    px[0].abs_diff(GPU_CLEAR_RGBA[0]) <= 6
        && px[1].abs_diff(GPU_CLEAR_RGBA[1]) <= 6
        && px[2].abs_diff(GPU_CLEAR_RGBA[2]) <= 6
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
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
