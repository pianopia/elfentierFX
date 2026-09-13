//! Static colliders for smoke and FLIP liquid solvers (AABB box or mesh SDF).

use crate::mesh::Vec3;
use crate::mesh_sdf::{
    build_mesh_sdf, create_collider_mesh, mesh_wireframe_segments, transform_collider_mesh,
    ColliderMeshKind, MeshSdf,
};
use serde::{Deserialize, Serialize};

/// Collider shape mode: axis-aligned box or voxelized mesh SDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColliderMode {
    #[default]
    Aabb,
    MeshSdf,
}

/// Static collider wired through the procedural graph.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColliderInput {
    pub enabled: bool,
    #[serde(default)]
    pub mode: ColliderMode,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    /// Restitution for liquid bounce (0 = stick/slide, 1 = full reflect).
    pub bounce: f32,
    /// When true, smoke density and liquid particles inside the volume are removed.
    pub kill_inside: bool,
    /// Procedural mesh kind when `mode == mesh_sdf`.
    #[serde(default)]
    pub mesh_kind: ColliderMeshKind,
    /// Voxel resolution per axis for mesh SDF (8–96).
    #[serde(default = "default_mesh_resolution")]
    pub mesh_resolution: u32,
    /// World position for mesh SDF colliders.
    #[serde(default)]
    pub position: Vec3,
    /// Y-axis rotation in radians for mesh SDF colliders.
    #[serde(default)]
    pub rotation_y: f32,
    /// Non-uniform scale for mesh SDF colliders.
    #[serde(default = "default_collider_scale")]
    pub scale: Vec3,
}

fn default_mesh_resolution() -> u32 {
    32
}

fn default_collider_scale() -> Vec3 {
    Vec3::new(1.0, 1.0, 1.0)
}

impl Default for ColliderInput {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: ColliderMode::Aabb,
            bounds_min: Vec3::new(-4.0, 0.0, -4.0),
            bounds_max: Vec3::new(4.0, 0.35, 4.0),
            bounce: 0.15,
            kill_inside: true,
            mesh_kind: ColliderMeshKind::Box,
            mesh_resolution: 32,
            position: Vec3::ZERO,
            rotation_y: 0.0,
            scale: Vec3::new(1.0, 1.0, 1.0),
        }
    }
}

impl ColliderInput {
    /// Flat floor slab spanning XZ at `y = 0`.
    pub fn floor(thickness: f32, half_extent: f32) -> Self {
        Self {
            enabled: true,
            mode: ColliderMode::Aabb,
            bounds_min: Vec3::new(-half_extent, 0.0, -half_extent),
            bounds_max: Vec3::new(half_extent, thickness, half_extent),
            bounce: 0.12,
            kill_inside: true,
            ..Default::default()
        }
    }

    /// Vertical wall on the −Z face of the domain.
    pub fn back_wall(thickness: f32, half_extent: f32, height: f32) -> Self {
        Self {
            enabled: true,
            mode: ColliderMode::Aabb,
            bounds_min: Vec3::new(-half_extent, 0.0, -half_extent - thickness),
            bounds_max: Vec3::new(half_extent, height, -half_extent),
            bounce: 0.2,
            kill_inside: true,
            ..Default::default()
        }
    }

    /// Sphere mesh SDF centered at `position` with uniform `radius`.
    pub fn sphere(position: Vec3, radius: f32, resolution: u32) -> Self {
        let r = radius.max(0.05);
        Self {
            enabled: true,
            mode: ColliderMode::MeshSdf,
            bounds_min: position.sub(Vec3::new(r, r, r)),
            bounds_max: position.add(Vec3::new(r, r, r)),
            bounce: 0.12,
            kill_inside: true,
            mesh_kind: ColliderMeshKind::Sphere,
            mesh_resolution: resolution,
            position,
            rotation_y: 0.0,
            scale: Vec3::new(r * 2.0, r * 2.0, r * 2.0),
        }
    }

    /// Torus mesh SDF for wrapping obstacles.
    pub fn torus(position: Vec3, major_radius: f32, minor_radius: f32, resolution: u32) -> Self {
        let ext = major_radius + minor_radius;
        Self {
            enabled: true,
            mode: ColliderMode::MeshSdf,
            bounds_min: position.sub(Vec3::new(ext, minor_radius, ext)),
            bounds_max: position.add(Vec3::new(ext, minor_radius, ext)),
            bounce: 0.15,
            kill_inside: true,
            mesh_kind: ColliderMeshKind::Torus,
            mesh_resolution: resolution,
            position,
            rotation_y: 0.0,
            scale: Vec3::new(major_radius / 0.55, minor_radius / 0.18, major_radius / 0.55),
        }
    }

    /// Inclined ramp mesh SDF (local +Z is uphill).
    pub fn ramp(position: Vec3, width: f32, height: f32, depth: f32, resolution: u32) -> Self {
        Self {
            enabled: true,
            mode: ColliderMode::MeshSdf,
            bounds_min: Vec3::new(
                position.x - width * 0.5,
                position.y,
                position.z - depth * 0.5,
            ),
            bounds_max: Vec3::new(
                position.x + width * 0.5,
                position.y + height,
                position.z + depth * 0.5,
            ),
            bounce: 0.1,
            kill_inside: true,
            mesh_kind: ColliderMeshKind::Ramp,
            mesh_resolution: resolution,
            position,
            rotation_y: 0.0,
            scale: Vec3::new(width / 2.0, height / 0.6, depth / 2.0),
        }
    }

    pub fn is_mesh_sdf(&self) -> bool {
        self.enabled && self.mode == ColliderMode::MeshSdf
    }

    pub fn contains(&self, p: Vec3) -> bool {
        if !self.enabled {
            return false;
        }
        if self.mode == ColliderMode::MeshSdf {
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

/// Resolved collider with optional baked mesh SDF.
#[derive(Debug, Clone)]
pub struct ResolvedCollider {
    pub input: ColliderInput,
    pub sdf: Option<MeshSdf>,
}

impl ResolvedCollider {
    pub fn resolve(input: ColliderInput) -> Self {
        let sdf = if input.is_mesh_sdf() {
            let local = create_collider_mesh(input.mesh_kind);
            let world = transform_collider_mesh(
                &local,
                input.position,
                input.rotation_y,
                input.scale,
            );
            Some(build_mesh_sdf(&world, input.mesh_resolution))
        } else {
            None
        };
        Self { input, sdf }
    }

    pub fn contains(&self, p: Vec3) -> bool {
        if !self.input.enabled {
            return false;
        }
        if let Some(sdf) = &self.sdf {
            sdf.contains(p)
        } else {
            self.input.contains(p)
        }
    }

    pub fn sample_distance(&self, p: Vec3) -> Option<f32> {
        self.sdf.as_ref().map(|sdf| sdf.sample(p))
    }

    pub fn gradient(&self, p: Vec3) -> Option<Vec3> {
        self.sdf.as_ref().map(|sdf| sdf.gradient(p))
    }
}

/// Builds resolved colliders (mesh SDFs baked once per cook).
pub fn resolve_colliders(colliders: &[ColliderInput]) -> Vec<ResolvedCollider> {
    colliders
        .iter()
        .copied()
        .map(ResolvedCollider::resolve)
        .collect()
}

/// Wireframe line pairs (two points per segment) for viewport display.
pub fn wireframe_segments(collider: &ColliderInput) -> Vec<[f32; 3]> {
    if !collider.enabled {
        return Vec::new();
    }
    if collider.mode == ColliderMode::MeshSdf {
        let local = create_collider_mesh(collider.mesh_kind);
        let world = transform_collider_mesh(
            &local,
            collider.position,
            collider.rotation_y,
            collider.scale,
        );
        return mesh_wireframe_segments(&world);
    }

    wireframe_aabb(collider)
}

fn wireframe_aabb(collider: &ColliderInput) -> Vec<[f32; 3]> {
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

/// Viewport bounds for a collider (AABB or mesh SDF grid bounds).
pub fn collider_display_bounds(collider: &ColliderInput) -> (Vec3, Vec3) {
    if collider.mode == ColliderMode::MeshSdf {
        let resolved = ResolvedCollider::resolve(*collider);
        if let Some(sdf) = resolved.sdf {
            return (sdf.bounds_min, sdf.bounds_max);
        }
    }
    (collider.bounds_min, collider.bounds_max)
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
    let resolved = resolve_colliders(colliders);
    apply_colliders_smoke_resolved(
        nx,
        ny,
        nz,
        bounds_min,
        _bounds_max,
        voxel_size,
        density,
        vel_x,
        vel_y,
        vel_z,
        temperature,
        &resolved,
    );
}

pub fn apply_colliders_smoke_resolved(
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
    colliders: &[ResolvedCollider],
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
        if !collider.input.enabled {
            continue;
        }

        if let Some(sdf) = &collider.sdf {
            let band = sdf.voxel_size().x.max(sdf.voxel_size().y).max(sdf.voxel_size().z) * 0.75;
            for k in 0..nz {
                for j in 0..ny {
                    for i in 0..nx {
                        let idx = index(i, j, k);
                        let world = grid_to_world(i as f32, j as f32, k as f32);
                        let dist = sdf.sample(world);
                        if dist < 0.0 {
                            if collider.input.kill_inside {
                                density[idx] = 0.0;
                                temperature[idx] = 0.0;
                            }
                            vel_x[idx] = 0.0;
                            vel_y[idx] = 0.0;
                            vel_z[idx] = 0.0;
                        } else if dist < band {
                            let grad = sdf.gradient(world);
                            let vx = vel_x[idx];
                            let vy = vel_y[idx];
                            let vz = vel_z[idx];
                            let vn = vx * grad.x + vy * grad.y + vz * grad.z;
                            if vn < 0.0 {
                                vel_x[idx] = vx - vn * grad.x;
                                vel_y[idx] = vy - vn * grad.y;
                                vel_z[idx] = vz - vn * grad.z;
                            }
                        }
                    }
                }
            }
            continue;
        }

        apply_aabb_smoke_cell(
            collider,
            nx,
            ny,
            nz,
            voxel_size,
            density,
            vel_x,
            vel_y,
            vel_z,
            temperature,
            &index,
            &grid_to_world,
        );
    }
}

fn apply_aabb_smoke_cell(
    collider: &ResolvedCollider,
    nx: usize,
    ny: usize,
    nz: usize,
    voxel_size: Vec3,
    density: &mut [f32],
    vel_x: &mut [f32],
    vel_y: &mut [f32],
    vel_z: &mut [f32],
    temperature: &mut [f32],
    index: &dyn Fn(usize, usize, usize) -> usize,
    grid_to_world: &dyn Fn(f32, f32, f32) -> Vec3,
) {
    let input = collider.input;
    let pad_x = voxel_size.x * 0.55;
    let pad_y = voxel_size.y * 0.55;
    let pad_z = voxel_size.z * 0.55;

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let idx = index(i, j, k);
                let world = grid_to_world(i as f32, j as f32, k as f32);

                if collider.contains(world) {
                    if input.kill_inside {
                        density[idx] = 0.0;
                        temperature[idx] = 0.0;
                    }
                    vel_x[idx] = 0.0;
                    vel_y[idx] = 0.0;
                    vel_z[idx] = 0.0;
                    continue;
                }

                let mn = input.bounds_min;
                let mx = input.bounds_max;

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

/// Liquid particle position/velocity response to colliders.
pub fn resolve_particle_colliders(
    pos: &mut Vec3,
    vel: &mut Vec3,
    radius: f32,
    colliders: &[ColliderInput],
) {
    let resolved = resolve_colliders(colliders);
    resolve_particle_colliders_resolved(pos, vel, radius, &resolved);
}

pub fn resolve_particle_colliders_resolved(
    pos: &mut Vec3,
    vel: &mut Vec3,
    radius: f32,
    colliders: &[ResolvedCollider],
) {
    for collider in colliders {
        if !collider.input.enabled {
            continue;
        }
        if collider.sdf.is_some() {
            resolve_particle_sdf(pos, vel, radius, collider);
        } else {
            resolve_particle_aabb(pos, vel, radius, &collider.input);
        }
    }
}

fn resolve_particle_sdf(pos: &mut Vec3, vel: &mut Vec3, radius: f32, collider: &ResolvedCollider) {
    let sdf = collider.sdf.as_ref().expect("mesh sdf");
    let bounce = collider.input.bounce.clamp(0.0, 1.0);
    let dist = sdf.sample(*pos);

    if dist < radius {
        if dist < 0.0 && collider.input.kill_inside {
            let grad = sdf.gradient(*pos);
            *pos = pos.add(grad.scale(radius - dist));
            let vn = vel.x * grad.x + vel.y * grad.y + vel.z * grad.z;
            if vn < 0.0 {
                *vel = vel.sub(grad.scale(vn * (1.0 + bounce)));
            }
            return;
        }

        let grad = sdf.gradient(*pos);
        let push = radius - dist;
        *pos = pos.add(grad.scale(push));

        let vn = vel.x * grad.x + vel.y * grad.y + vel.z * grad.z;
        if vn < 0.0 {
            *vel = vel.sub(grad.scale(vn * (1.0 + bounce)));
        }
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
    let resolved = resolve_colliders(colliders);
    apply_colliders_liquid_grid_resolved(
        nx,
        ny,
        nz,
        bounds_min,
        _bounds_max,
        voxel_size,
        weight,
        vel_x,
        vel_y,
        vel_z,
        &resolved,
    );
}

pub fn apply_colliders_liquid_grid_resolved(
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
    colliders: &[ResolvedCollider],
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
        if !collider.input.enabled {
            continue;
        }

        if let Some(sdf) = &collider.sdf {
            let band = sdf.voxel_size().x.max(sdf.voxel_size().y).max(sdf.voxel_size().z) * 0.75;
            for k in 0..nz {
                for j in 0..ny {
                    for i in 0..nx {
                        let idx = index(i, j, k);
                        if weight[idx] < 1e-6 {
                            continue;
                        }
                        let world = grid_to_world(i as f32, j as f32, k as f32);
                        let dist = sdf.sample(world);
                        if dist < 0.0 {
                            vel_x[idx] = 0.0;
                            vel_y[idx] = 0.0;
                            vel_z[idx] = 0.0;
                        } else if dist < band {
                            let grad = sdf.gradient(world);
                            let vx = vel_x[idx];
                            let vy = vel_y[idx];
                            let vz = vel_z[idx];
                            let vn = vx * grad.x + vy * grad.y + vz * grad.z;
                            if vn < 0.0 {
                                vel_x[idx] = vx - vn * grad.x;
                                vel_y[idx] = vy - vn * grad.y;
                                vel_z[idx] = vz - vn * grad.z;
                            }
                        }
                    }
                }
            }
            continue;
        }

        apply_aabb_liquid_grid_cell(
            collider,
            nx,
            ny,
            nz,
            voxel_size,
            weight,
            vel_x,
            vel_y,
            vel_z,
            &index,
            &grid_to_world,
        );
    }
}

fn apply_aabb_liquid_grid_cell(
    collider: &ResolvedCollider,
    nx: usize,
    ny: usize,
    nz: usize,
    voxel_size: Vec3,
    weight: &[f32],
    vel_x: &mut [f32],
    vel_y: &mut [f32],
    vel_z: &mut [f32],
    index: &dyn Fn(usize, usize, usize) -> usize,
    grid_to_world: &dyn Fn(f32, f32, f32) -> Vec3,
) {
    let input = collider.input;
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

                let mn = input.bounds_min;
                let mx = input.bounds_max;
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
    fn wireframe_has_twenty_four_points_for_aabb() {
        let segs = wireframe_segments(&ColliderInput::default());
        assert_eq!(segs.len(), 24);
    }

    #[test]
    fn wireframe_has_more_segments_for_sphere_mesh() {
        let sphere = ColliderInput::sphere(Vec3::new(0.0, 1.0, 0.0), 0.8, 24);
        let segs = wireframe_segments(&sphere);
        assert!(segs.len() > 24);
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

    #[test]
    fn particle_pushed_out_of_sphere_sdf() {
        let sphere = ColliderInput::sphere(Vec3::new(0.0, 1.0, 0.0), 0.6, 28);
        let mut pos = Vec3::new(0.0, 1.45, 0.0);
        let mut vel = Vec3::new(0.0, -2.0, 0.0);
        resolve_particle_colliders(&mut pos, &mut vel, 0.08, &[sphere]);
        let resolved = ResolvedCollider::resolve(sphere);
        let dist = resolved.sdf.unwrap().sample(pos);
        assert!(dist >= 0.0, "particle should sit on or outside sphere, dist={dist}");
    }

    #[test]
    fn smoke_density_cleared_inside_sphere_sdf() {
        let sphere = ColliderInput::sphere(Vec3::new(0.0, 2.0, 0.0), 0.7, 24);
        let mut density = vec![1.0; 1];
        let mut vel_x = vec![0.0; 1];
        let mut vel_y = vec![-1.0; 1];
        let mut vel_z = vec![0.0; 1];
        let mut temp = vec![1.0; 1];
        apply_colliders_smoke(
            1,
            1,
            1,
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(1.0, 3.0, 1.0),
            Vec3::new(2.0, 2.0, 2.0),
            &mut density,
            &mut vel_x,
            &mut vel_y,
            &mut vel_z,
            &mut temp,
            &[sphere],
        );
        assert!(density[0] < 0.01);
        assert_eq!(vel_y[0], 0.0);
    }

    #[test]
    fn mesh_sdf_smoke_simulation_runs() {
        use crate::smoke::{simulate_smoke, SmokeDomainInput, SmokeSolverInput, SmokeSourceInput};

        let domain = SmokeDomainInput {
            resolution: 14,
            ..Default::default()
        };
        let sources = vec![SmokeSourceInput {
            position: Vec3::new(0.0, 1.0, 0.0),
            ..Default::default()
        }];
        let solver = SmokeSolverInput {
            steps: 8,
            frame_stride: 8,
            ground_collision: false,
            ..Default::default()
        };
        let sphere = ColliderInput::sphere(Vec3::new(0.0, 0.5, 0.0), 0.5, 20);
        let vol = simulate_smoke(&domain, &sources, &solver, &[sphere]);
        assert!(vol.stats.max_density > 0.0);
    }
}
