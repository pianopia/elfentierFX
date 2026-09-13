//! Instance placement along paths and grid lots.

use crate::mesh::Vec3;
use serde::{Deserialize, Serialize};

/// Transform for one placed building instance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InstanceTransform {
    pub position: Vec3,
    pub rotation_y: f32,
    pub scale: Vec3,
}

impl Default for InstanceTransform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation_y: 0.0,
            scale: Vec3::new(1.0, 1.0, 1.0),
        }
    }
}

/// Straight path input for `place_along_path`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PathInput {
    pub start: Vec3,
    pub end: Vec3,
    pub spacing: f32,
    /// Lateral offset from path centerline (positive = left when facing end).
    pub offset_from_path: f32,
}

impl Default for PathInput {
    fn default() -> Self {
        Self {
            start: Vec3::new(0.0, 0.0, 0.0),
            end: Vec3::new(40.0, 0.0, 0.0),
            spacing: 8.0,
            offset_from_path: 0.0,
        }
    }
}

/// Grid lot input for `fill_grid`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridInput {
    pub origin: Vec3,
    pub cols: u32,
    pub rows: u32,
    pub lot_width: f32,
    pub lot_depth: f32,
    pub spacing: f32,
}

impl Default for GridInput {
    fn default() -> Self {
        Self {
            origin: Vec3::ZERO,
            cols: 3,
            rows: 2,
            lot_width: 8.0,
            lot_depth: 8.0,
            spacing: 1.5,
        }
    }
}

/// Places instances along a straight path at regular spacing.
pub fn place_along_path(path: &PathInput, seed: u64) -> Vec<InstanceTransform> {
    let spacing = path.spacing.max(0.5);
    let delta = path.end.sub(path.start);
    let length = delta.length();
    if length < 1e-4 {
        return vec![InstanceTransform::default()];
    }

    let dir = delta.normalize();
    let count = ((length / spacing).floor() as u32).max(1);
    let lateral = Vec3::new(-dir.z, 0.0, dir.x).scale(path.offset_from_path);

    let mut transforms = Vec::with_capacity(count as usize);
    for i in 0..count {
        let t = if count == 1 {
            0.5
        } else {
            i as f32 / (count - 1) as f32
        };
        let along = dir.scale(length * t);
        let jitter = placement_jitter(seed, i) * 0.15;
        let scale_var = 1.0 + placement_jitter(seed ^ 0xdead, i) * 0.08;
        transforms.push(InstanceTransform {
            position: path.start.add(along).add(lateral).add(Vec3::new(0.0, 0.0, jitter)),
            rotation_y: if path.offset_from_path.abs() > 0.01 {
                std::f32::consts::FRAC_PI_2
            } else {
                0.0
            },
            scale: Vec3::new(scale_var, 1.0, scale_var),
        });
    }
    transforms
}

/// Fills a rectangular grid of lot centers with instances.
pub fn fill_grid(grid: &GridInput, seed: u64) -> Vec<InstanceTransform> {
    let cols = grid.cols.max(1);
    let rows = grid.rows.max(1);
    let step_x = grid.lot_width + grid.spacing;
    let step_z = grid.lot_depth + grid.spacing;

    let mut transforms = Vec::with_capacity((cols * rows) as usize);
    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col;
            let jitter = placement_jitter(seed, idx);
            let rot = if idx % 2 == 0 { 0.0 } else { std::f32::consts::PI };
            transforms.push(InstanceTransform {
                position: Vec3::new(
                    grid.origin.x + col as f32 * step_x + jitter * 0.2,
                    grid.origin.y,
                    grid.origin.z + row as f32 * step_z + jitter * 0.2,
                ),
                rotation_y: rot,
                scale: Vec3::new(1.0 + jitter * 0.05, 1.0, 1.0 + jitter * 0.05),
            });
        }
    }
    transforms
}

fn placement_jitter(seed: u64, index: u32) -> f32 {
    let mut x = seed ^ (index as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afed_0b33_aaad);
    x ^= x >> 33;
    let unit = (x & 0xffff) as f32 / 65535.0;
    unit * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_placement_count() {
        let path = PathInput {
            start: Vec3::ZERO,
            end: Vec3::new(32.0, 0.0, 0.0),
            spacing: 8.0,
            offset_from_path: 0.0,
        };
        let instances = place_along_path(&path, 1);
        assert_eq!(instances.len(), 4);
    }

    #[test]
    fn grid_fill_count() {
        let grid = GridInput {
            origin: Vec3::ZERO,
            cols: 3,
            rows: 2,
            lot_width: 6.0,
            lot_depth: 6.0,
            spacing: 1.0,
        };
        let instances = fill_grid(&grid, 7);
        assert_eq!(instances.len(), 6);
    }

    #[test]
    fn path_zero_length_returns_one() {
        let path = PathInput {
            start: Vec3::new(1.0, 0.0, 1.0),
            end: Vec3::new(1.0, 0.0, 1.0),
            spacing: 5.0,
            offset_from_path: 0.0,
        };
        assert_eq!(place_along_path(&path, 0).len(), 1);
    }
}
