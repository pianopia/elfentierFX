//! Procedural building mesh generation.

use crate::mesh::{Mesh, Vec3};
use serde::{Deserialize, Serialize};

/// Parameters controlling procedural building generation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BuildingParams {
    pub floors: u32,
    pub width: f32,
    pub depth: f32,
    /// Window coverage along each facade row, 0.0–1.0.
    pub window_density: f32,
    pub seed: u64,
    /// Height per floor in world units.
    pub floor_height: f32,
}

impl Default for BuildingParams {
    fn default() -> Self {
        Self {
            floors: 3,
            width: 8.0,
            depth: 6.0,
            window_density: 0.55,
            seed: 1,
            floor_height: 3.0,
        }
    }
}

impl BuildingParams {
    pub fn shop_preset() -> Self {
        Self {
            floors: 2,
            width: 6.0,
            depth: 5.0,
            window_density: 0.65,
            seed: 42,
            floor_height: 3.2,
        }
    }

    pub fn total_height(&self) -> f32 {
        self.floors as f32 * self.floor_height
    }
}

/// Generates a stylized building mesh: stacked floor slabs, facade window grid, simple roof cap.
pub fn generate_building(params: &BuildingParams) -> Mesh {
    let floors = params.floors.max(1);
    let width = params.width.max(1.0);
    let depth = params.depth.max(1.0);
    let floor_h = params.floor_height.max(0.5);
    let density = params.window_density.clamp(0.0, 1.0);

    let mut mesh = Mesh::with_name("building");
    let half_w = width * 0.5;
    let half_d = depth * 0.5;
    let total_h = floors as f32 * floor_h;

    // Main volume
    mesh.add_box(
        Vec3::new(-half_w, 0.0, -half_d),
        Vec3::new(half_w, total_h, half_d),
    );

    // Facade window recesses on +Z face
    let cols = ((width / 2.0) * density).round().max(1.0) as u32;
    let rows = ((floors as f32) * density).round().max(1.0) as u32;
    let recess_depth = 0.12;
    let margin_x = width * 0.12;
    let margin_y = floor_h * 0.2;
    let usable_w = width - margin_x * 2.0;
    let cell_w = usable_w / cols as f32;
    let cell_h = (total_h - margin_y * 2.0) / rows as f32;
    let win_w = cell_w * 0.55;
    let win_h = cell_h * 0.55;

    for row in 0..rows {
        for col in 0..cols {
            if !window_visible(params.seed, row, col, density) {
                continue;
            }
            let cx = -half_w + margin_x + cell_w * (col as f32 + 0.5);
            let cy = margin_y + cell_h * (row as f32 + 0.5);
            let z = half_d - recess_depth;
            add_window_recess(
                &mut mesh,
                cx,
                cy,
                z,
                half_d,
                win_w,
                win_h,
                recess_depth,
            );
        }
    }

    // Simple roof cap (slightly wider flat slab)
    let roof_overhang = 0.25;
    let roof_h = floor_h * 0.25;
    mesh.add_box(
        Vec3::new(-half_w - roof_overhang, total_h, -half_d - roof_overhang),
        Vec3::new(
            half_w + roof_overhang,
            total_h + roof_h,
            half_d + roof_overhang,
        ),
    );

    mesh.vertices.byte_len = mesh.positions.len() * 12;
    mesh.indices_buffer.byte_len = mesh.indices.len() * 4;
    mesh
}

fn window_visible(seed: u64, row: u32, col: u32, density: f32) -> bool {
    let hash = hash_coords(seed, row, col);
    let threshold = (density * 255.0) as u32;
    hash <= threshold
}

fn hash_coords(seed: u64, row: u32, col: u32) -> u32 {
    let mut x = seed ^ ((row as u64) << 32) ^ (col as u64);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afed_0b33_aaad);
    x ^= x >> 33;
    (x & 0xff) as u32
}

fn add_window_recess(
    mesh: &mut Mesh,
    cx: f32,
    cy: f32,
    z_front: f32,
    z_back: f32,
    win_w: f32,
    win_h: f32,
    recess_depth: f32,
) {
    let hw = win_w * 0.5;
    let hh = win_h * 0.5;
    let z_inner = z_front - recess_depth;
    mesh.add_quad(
        Vec3::new(cx - hw, cy - hh, z_front),
        Vec3::new(cx + hw, cy - hh, z_front),
        Vec3::new(cx + hw, cy + hh, z_front),
        Vec3::new(cx - hw, cy + hh, z_front),
    );
    mesh.add_quad(
        Vec3::new(cx - hw, cy - hh, z_inner),
        Vec3::new(cx - hw, cy + hh, z_inner),
        Vec3::new(cx + hw, cy + hh, z_inner),
        Vec3::new(cx + hw, cy - hh, z_inner),
    );
    mesh.add_quad(
        Vec3::new(cx - hw, cy - hh, z_inner),
        Vec3::new(cx + hw, cy - hh, z_inner),
        Vec3::new(cx + hw, cy - hh, z_front),
        Vec3::new(cx - hw, cy - hh, z_front),
    );
    mesh.add_quad(
        Vec3::new(cx - hw, cy + hh, z_inner),
        Vec3::new(cx - hw, cy + hh, z_front),
        Vec3::new(cx + hw, cy + hh, z_front),
        Vec3::new(cx + hw, cy + hh, z_inner),
    );
    mesh.add_quad(
        Vec3::new(cx - hw, cy - hh, z_inner),
        Vec3::new(cx - hw, cy - hh, z_back),
        Vec3::new(cx - hw, cy + hh, z_back),
        Vec3::new(cx - hw, cy + hh, z_inner),
    );
    mesh.add_quad(
        Vec3::new(cx + hw, cy - hh, z_inner),
        Vec3::new(cx + hw, cy + hh, z_inner),
        Vec3::new(cx + hw, cy + hh, z_back),
        Vec3::new(cx + hw, cy - hh, z_back),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_has_geometry() {
        let params = BuildingParams::default();
        let mesh = generate_building(&params);
        assert!(mesh.vertex_count() > 8);
        assert!(mesh.triangle_count() > 12);
    }

    #[test]
    fn shop_preset_differs_by_seed() {
        let a = generate_building(&BuildingParams::shop_preset());
        let mut b_params = BuildingParams::shop_preset();
        b_params.seed = 99;
        let b = generate_building(&b_params);
        assert_ne!(a.positions.len(), b.positions.len());
    }

    #[test]
    fn more_floors_increase_height() {
        let low = generate_building(&BuildingParams {
            floors: 1,
            width: 4.0,
            depth: 4.0,
            window_density: 0.0,
            seed: 1,
            floor_height: 3.0,
        });
        let high = generate_building(&BuildingParams {
            floors: 4,
            width: 4.0,
            depth: 4.0,
            window_density: 0.0,
            seed: 1,
            floor_height: 3.0,
        });
        let low_max_y = low
            .positions
            .iter()
            .map(|p| p.y)
            .fold(0.0_f32, f32::max);
        let high_max_y = high
            .positions
            .iter()
            .map(|p| p.y)
            .fold(0.0_f32, f32::max);
        assert!(high_max_y > low_max_y);
    }
}
