//! Axis-aligned box colliders for smoke and FLIP liquid solvers.

use crate::mesh::Vec3;
use serde::{Deserialize, Serialize};

/// Static AABB collider wired through the procedural graph.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColliderInput {
    pub enabled: bool,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    /// Restitution for liquid bounce (0 = stick/slide, 1 = full reflect).
    pub bounce: f32,
    /// When true, smoke density and liquid particles inside the volume are removed.
    pub kill_inside: bool,
}

impl Default for ColliderInput {
    fn default() -> Self {
        Self {
            enabled: true,
            bounds_min: Vec3::new(-4.0, 0.0, -4.0),
            bounds_max: Vec3::new(4.0, 0.35, 4.0),
            bounce: 0.15,
            kill_inside: true,
        }
    }
}

impl ColliderInput {
    /// Flat floor slab spanning XZ at `y = 0`.
    pub fn floor(thickness: f32, half_extent: f32) -> Self {
        Self {
            enabled: true,
            bounds_min: Vec3::new(-half_extent, 0.0, -half_extent),
            bounds_max: Vec3::new(half_extent, thickness, half_extent),
            bounce: 0.12,
            kill_inside: true,
        }
    }

    /// Vertical wall on the −Z face of the domain.
    pub fn back_wall(thickness: f32, half_extent: f32, height: f32) -> Self {
        Self {
            enabled: true,
            bounds_min: Vec3::new(-half_extent, 0.0, -half_extent - thickness),
            bounds_max: Vec3::new(half_extent, height, -half_extent),
            bounce: 0.2,
            kill_inside: true,
        }
    }

    pub fn contains(&self, p: Vec3) -> bool {
        if !self.enabled {
            return false;
        }
        p.x >= self.bounds_min.x
            && p.x <= self.bounds_max.x
            && p.y >= self.bounds_min.y
            && p.y <= self.bounds_max.y
            && p.z >= self.bounds_min.z
            && p.z <= self.bounds_max.z
    }
}

/// Wireframe line pairs (two points per segment) for viewport display.
pub fn wireframe_segments(collider: &ColliderInput) -> Vec<[f32; 3]> {
    if !collider.enabled {
        return Vec::new();
    }
    let mn = collider.bounds_min;
    let mx = collider.bounds_max;
    let corners = [
        Vec3::new(mn.x, mn.y, mn.z),
        Vec3::new(mx.x, mn.y, mn.z),
        Vec3::new(mx.x, mn.y, mx.z),
        Vec3::new(mn.x, mn.y, mx.z),
        Vec3::new(mn.x, mx.y, mn.z),
        Vec3::new(mx.x, mx.y, mn.z),
        Vec3::new(mx.x, mx.y, mx.z),
        Vec3::new(mn.x, mx.y, mx.z),
    ];
    let edges: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let mut lines = Vec::with_capacity(edges.len() * 2);
    for (a, b) in edges {
        let pa = corners[a];
        let pb = corners[b];
        lines.push([pa.x, pa.y, pa.z]);
        lines.push([pb.x, pb.y, pb.z]);
    }
    lines
}

/// Collects wireframe segments from all enabled colliders.
pub fn all_wireframe_segments(colliders: &[ColliderInput]) -> Vec<[f32; 3]> {
    colliders
        .iter()
        .flat_map(wireframe_segments)
        .collect()
}

/// Smoke grid: block interior and clamp inward velocity at faces.
pub fn apply_colliders_smoke(
    nx: usize,
    ny: usize,
    nz: usize,
    bounds_min: Vec3,
    _bounds_max: Vec3,
    voxel_size: Vec3,
    density: &mut [f32],
    vel_x: &mut [f32],
    vel_y: &mut [f32],
    vel_z: &mut [f32],
    temperature: &mut [f32],
    colliders: &[ColliderInput],
) {
    let index = |i: usize, j: usize, k: usize| i + nx * (j + ny * k);
    let grid_to_world = |i: f32, j: f32, k: f32| -> Vec3 {
        Vec3::new(
            bounds_min.x + (i + 0.5) * voxel_size.x,
            bounds_min.y + (j + 0.5) * voxel_size.y,
            bounds_min.z + (k + 0.5) * voxel_size.z,
        )
    };

    for collider in colliders {
        if !collider.enabled {
            continue;
        }
        let pad_x = voxel_size.x * 0.55;
        let pad_y = voxel_size.y * 0.55;
        let pad_z = voxel_size.z * 0.55;

        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let idx = index(i, j, k);
                    let world = grid_to_world(i as f32, j as f32, k as f32);

                    if collider.contains(world) {
                        if collider.kill_inside {
                            density[idx] = 0.0;
                            temperature[idx] = 0.0;
                        }
                        vel_x[idx] = 0.0;
                        vel_y[idx] = 0.0;
                        vel_z[idx] = 0.0;
                        continue;
                    }

                    let mn = collider.bounds_min;
                    let mx = collider.bounds_max;

                    if world.x >= mn.x - pad_x
                        && world.x <= mx.x + pad_x
                        && world.y >= mn.y - pad_y
                        && world.y <= mx.y + pad_y
                        && world.z >= mn.z - pad_z
                        && world.z <= mx.z + pad_z
                    {
                        if world.x < mn.x && vel_x[idx] < 0.0 {
                            vel_x[idx] = 0.0;
                        }
                        if world.x > mx.x && vel_x[idx] > 0.0 {
                            vel_x[idx] = 0.0;
                        }
                        if world.y < mn.y && vel_y[idx] < 0.0 {
                            vel_y[idx] = 0.0;
                        }
                        if world.y > mx.y && vel_y[idx] > 0.0 {
                            vel_y[idx] = 0.0;
                        }
                        if world.z < mn.z && vel_z[idx] < 0.0 {
                            vel_z[idx] = 0.0;
                        }
                        if world.z > mx.z && vel_z[idx] > 0.0 {
                            vel_z[idx] = 0.0;
                        }
                    }
                }
            }
        }
    }
}

/// Liquid particle position/velocity response to AABB colliders.
pub fn resolve_particle_colliders(
    pos: &mut Vec3,
    vel: &mut Vec3,
    radius: f32,
    colliders: &[ColliderInput],
) {
    for collider in colliders {
        if !collider.enabled {
            continue;
        }
        resolve_particle_aabb(pos, vel, radius, collider);
    }
}

fn resolve_particle_aabb(pos: &mut Vec3, vel: &mut Vec3, radius: f32, collider: &ColliderInput) {
    let mn = collider.bounds_min;
    let mx = collider.bounds_max;
    let bounce = collider.bounce.clamp(0.0, 1.0);

    if pos.x + radius <= mn.x
        || pos.x - radius >= mx.x
        || pos.y + radius <= mn.y
        || pos.y - radius >= mx.y
        || pos.z + radius <= mn.z
        || pos.z - radius >= mx.z
    {
        return;
    }

    if collider.contains(*pos) && collider.kill_inside {
        let size_x = mx.x - mn.x;
        let size_y = mx.y - mn.y;
        let size_z = mx.z - mn.z;
        let slab_y = size_y <= size_x.min(size_z);

        if slab_y {
            pos.y = mx.y + radius;
            if vel.y < 0.0 {
                vel.y = -vel.y * bounce;
            }
        } else {
            let pen_x = (pos.x - mn.x).min(mx.x - pos.x);
            let pen_y = (pos.y - mn.y).min(mx.y - pos.y);
            let pen_z = (pos.z - mn.z).min(mx.z - pos.z);
            if pen_y <= pen_x && pen_y <= pen_z {
                if vel.y <= 0.0 {
                    pos.y = mx.y + radius;
                    if vel.y < 0.0 {
                        vel.y = -vel.y * bounce;
                    }
                } else {
                    pos.y = mn.y - radius;
                    vel.y = -vel.y * bounce;
                }
            } else if pen_x <= pen_z {
                if vel.x <= 0.0 {
                    pos.x = mx.x + radius;
                    if vel.x < 0.0 {
                        vel.x = -vel.x * bounce;
                    }
                } else {
                    pos.x = mn.x - radius;
                    vel.x = -vel.x * bounce;
                }
            } else if vel.z <= 0.0 {
                pos.z = mx.z + radius;
                if vel.z < 0.0 {
                    vel.z = -vel.z * bounce;
                }
            } else {
                pos.z = mn.z - radius;
                vel.z = -vel.z * bounce;
            }
        }
        return;
    }

    if pos.x - radius < mn.x && pos.x + radius > mn.x {
        pos.x = mn.x - radius;
        if vel.x < 0.0 {
            vel.x = -vel.x * bounce;
        }
    } else if pos.x + radius > mx.x && pos.x - radius < mx.x {
        pos.x = mx.x + radius;
        if vel.x > 0.0 {
            vel.x = -vel.x * bounce;
        }
    }

    if pos.y - radius < mn.y && pos.y + radius > mn.y {
        pos.y = mn.y - radius;
        if vel.y < 0.0 {
            vel.y = -vel.y * bounce;
        }
    } else if pos.y + radius > mx.y && pos.y - radius < mx.y {
        pos.y = mx.y + radius;
        if vel.y > 0.0 {
            vel.y = -vel.y * bounce;
        }
    }

    if pos.z - radius < mn.z && pos.z + radius > mn.z {
        pos.z = mn.z - radius;
        if vel.z < 0.0 {
            vel.z = -vel.z * bounce;
        }
    } else if pos.z + radius > mx.z && pos.z - radius < mx.z {
        pos.z = mx.z + radius;
        if vel.z > 0.0 {
            vel.z = -vel.z * bounce;
        }
    }
}

/// Liquid grid velocity blocking at collider faces (mirrors smoke path).
pub fn apply_colliders_liquid_grid(
    nx: usize,
    ny: usize,
    nz: usize,
    bounds_min: Vec3,
    _bounds_max: Vec3,
    voxel_size: Vec3,
    weight: &[f32],
    vel_x: &mut [f32],
    vel_y: &mut [f32],
    vel_z: &mut [f32],
    colliders: &[ColliderInput],
) {
    let index = |i: usize, j: usize, k: usize| i + nx * (j + ny * k);
    let grid_to_world = |i: f32, j: f32, k: f32| -> Vec3 {
        Vec3::new(
            bounds_min.x + (i + 0.5) * voxel_size.x,
            bounds_min.y + (j + 0.5) * voxel_size.y,
            bounds_min.z + (k + 0.5) * voxel_size.z,
        )
    };

    for collider in colliders {
        if !collider.enabled {
            continue;
        }
        let pad_x = voxel_size.x * 0.55;
        let pad_y = voxel_size.y * 0.55;
        let pad_z = voxel_size.z * 0.55;

        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let idx = index(i, j, k);
                    if weight[idx] < 1e-6 {
                        continue;
                    }
                    let world = grid_to_world(i as f32, j as f32, k as f32);
                    if collider.contains(world) {
                        vel_x[idx] = 0.0;
                        vel_y[idx] = 0.0;
                        vel_z[idx] = 0.0;
                        continue;
                    }

                    let mn = collider.bounds_min;
                    let mx = collider.bounds_max;
                    if world.x >= mn.x - pad_x
                        && world.x <= mx.x + pad_x
                        && world.y >= mn.y - pad_y
                        && world.y <= mx.y + pad_y
                        && world.z >= mn.z - pad_z
                        && world.z <= mx.z + pad_z
                    {
                        if world.x < mn.x && vel_x[idx] < 0.0 {
                            vel_x[idx] = 0.0;
                        }
                        if world.x > mx.x && vel_x[idx] > 0.0 {
                            vel_x[idx] = 0.0;
                        }
                        if world.y < mn.y && vel_y[idx] < 0.0 {
                            vel_y[idx] = 0.0;
                        }
                        if world.y > mx.y && vel_y[idx] > 0.0 {
                            vel_y[idx] = 0.0;
                        }
                        if world.z < mn.z && vel_z[idx] < 0.0 {
                            vel_z[idx] = 0.0;
                        }
                        if world.z > mx.z && vel_z[idx] > 0.0 {
                            vel_z[idx] = 0.0;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_collider_contains_ground_point() {
        let floor = ColliderInput::floor(0.3, 4.0);
        assert!(floor.contains(Vec3::new(0.0, 0.15, 0.0)));
        assert!(!floor.contains(Vec3::new(0.0, 2.0, 0.0)));
    }

    #[test]
    fn wireframe_has_twenty_four_points() {
        let segs = wireframe_segments(&ColliderInput::default());
        assert_eq!(segs.len(), 24);
    }

    #[test]
    fn particle_pushed_out_of_floor() {
        let floor = ColliderInput::floor(0.4, 2.0);
        let mut pos = Vec3::new(0.0, 0.1, 0.0);
        let mut vel = Vec3::new(0.0, -3.0, 0.0);
        resolve_particle_colliders(&mut pos, &mut vel, 0.1, &[floor]);
        assert!(pos.y >= 0.4);
        assert!(vel.y >= 0.0);
    }
}
