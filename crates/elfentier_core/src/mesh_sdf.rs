//! Voxelized signed distance fields from triangle meshes.
//!
//! Builds a dense `f32` grid over the mesh bounds. Distance to the nearest triangle
//! surface is computed per voxel; sign comes from ray-parity inside tests (+X ray).
//! This is pragmatic and fast enough for smoke/liquid collision, but:
//! - Accuracy is limited by grid resolution (stair-stepping on curved surfaces).
//! - Thin features smaller than a voxel may be missed or merged.
//! - Sharp concavities can get slightly rounded at voxel scale.
//! - No narrow-band optimization yet — full dense grid only.

use crate::mesh::{Mesh, Vec3};
use serde::{Deserialize, Serialize};

/// Built-in collider mesh primitives (procedural triangle meshes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColliderMeshKind {
    #[default]
    Box,
    Sphere,
    Torus,
    Ramp,
}

/// Dense uniform SDF stored on a regular grid.
#[derive(Debug, Clone)]
pub struct MeshSdf {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub values: Vec<f32>,
    /// Source mesh used for wireframe display (world space).
    pub display_mesh: Mesh,
}

impl MeshSdf {
    pub fn voxel_size(&self) -> Vec3 {
        Vec3::new(
            (self.bounds_max.x - self.bounds_min.x) / self.nx as f32,
            (self.bounds_max.y - self.bounds_min.y) / self.ny as f32,
            (self.bounds_max.z - self.bounds_min.z) / self.nz as f32,
        )
    }

    fn index(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.nx * (j + self.ny * k)
    }

    fn grid_to_world(&self, i: f32, j: f32, k: f32) -> Vec3 {
        let vs = self.voxel_size();
        Vec3::new(
            self.bounds_min.x + (i + 0.5) * vs.x,
            self.bounds_min.y + (j + 0.5) * vs.y,
            self.bounds_min.z + (k + 0.5) * vs.z,
        )
    }

    /// Signed distance at a world-space point (negative = inside solid).
    pub fn sample(&self, p: Vec3) -> f32 {
        let vs = self.voxel_size();
        let fx = (p.x - self.bounds_min.x) / vs.x - 0.5;
        let fy = (p.y - self.bounds_min.y) / vs.y - 0.5;
        let fz = (p.z - self.bounds_min.z) / vs.z - 0.5;
        sample_trilinear(&self.values, self.nx, self.ny, self.nz, fx, fy, fz)
    }

    /// Approximate outward-pointing gradient (points toward increasing SDF / outside).
    pub fn gradient(&self, p: Vec3) -> Vec3 {
        let eps = self.voxel_size().x.max(self.voxel_size().y).max(self.voxel_size().z) * 0.5;
        let dx = self.sample(p.add(Vec3::new(eps, 0.0, 0.0))) - self.sample(p.sub(Vec3::new(eps, 0.0, 0.0)));
        let dy = self.sample(p.add(Vec3::new(0.0, eps, 0.0))) - self.sample(p.sub(Vec3::new(0.0, eps, 0.0)));
        let dz = self.sample(p.add(Vec3::new(0.0, 0.0, eps))) - self.sample(p.sub(Vec3::new(0.0, 0.0, eps)));
        Vec3::new(dx, dy, dz).normalize()
    }

    pub fn contains(&self, p: Vec3) -> bool {
        self.sample(p) < 0.0
    }
}

/// Builds a procedural collider mesh in local space (before transform).
pub fn create_collider_mesh(kind: ColliderMeshKind) -> Mesh {
    match kind {
        ColliderMeshKind::Box => create_unit_box_collider(),
        ColliderMeshKind::Sphere => create_sphere_mesh(0.5, 16, 12),
        ColliderMeshKind::Torus => create_torus_mesh(0.55, 0.18, 24, 12),
        ColliderMeshKind::Ramp => create_ramp_mesh(2.0, 0.6, 2.0),
    }
}

fn create_unit_box_collider() -> Mesh {
    let mut mesh = Mesh::with_name("collider_box");
    mesh.add_box(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5));
    mesh
}

/// UV-sphere centered at origin with given radius.
pub fn create_sphere_mesh(radius: f32, segments: u32, rings: u32) -> Mesh {
    let mut mesh = Mesh::with_name("collider_sphere");
    let seg = segments.max(3) as usize;
    let ring = rings.max(2) as usize;

    for r in 0..=ring {
        let v = r as f32 / ring as f32;
        let phi = v * std::f32::consts::PI;
        let y = phi.cos() * radius;
        let ring_r = phi.sin() * radius;
        for s in 0..seg {
            let u = s as f32 / seg as f32;
            let theta = u * std::f32::consts::TAU;
            mesh.positions.push(Vec3::new(
                ring_r * theta.cos(),
                y,
                ring_r * theta.sin(),
            ));
        }
    }

    let row = seg;
    for r in 0..ring {
        for s in 0..seg {
            let i0 = (r * row + s) as u32;
            let i1 = (r * row + (s + 1) % seg) as u32;
            let i2 = ((r + 1) * row + s) as u32;
            let i3 = ((r + 1) * row + (s + 1) % seg) as u32;
            if r > 0 {
                mesh.indices.extend([i0, i2, i1]);
            }
            if r + 1 < ring {
                mesh.indices.extend([i1, i2, i3]);
            }
        }
    }
    mesh
}

/// Torus in XZ plane, Y up.
pub fn create_torus_mesh(major_r: f32, minor_r: f32, segments: u32, tube_segments: u32) -> Mesh {
    let mut mesh = Mesh::with_name("collider_torus");
    let seg = segments.max(3) as usize;
    let tube = tube_segments.max(3) as usize;

    for i in 0..seg {
        let u = i as f32 / seg as f32 * std::f32::consts::TAU;
        let cu = u.cos();
        let su = u.sin();
        for j in 0..tube {
            let v = j as f32 / tube as f32 * std::f32::consts::TAU;
            let cv = v.cos();
            let sv = v.sin();
            let r = major_r + minor_r * cv;
            mesh.positions.push(Vec3::new(r * cu, minor_r * sv, r * su));
        }
    }

    for i in 0..seg {
        for j in 0..tube {
            let i0 = (i * tube + j) as u32;
            let i1 = (i * tube + (j + 1) % tube) as u32;
            let i2 = (((i + 1) % seg) * tube + j) as u32;
            let i3 = (((i + 1) % seg) * tube + (j + 1) % tube) as u32;
            mesh.indices.extend([i0, i2, i1, i1, i2, i3]);
        }
    }
    mesh
}

/// Inclined wedge ramp: rises along +Z, flat base at y=0.
pub fn create_ramp_mesh(width: f32, height: f32, depth: f32) -> Mesh {
    let mut mesh = Mesh::with_name("collider_ramp");
    let hw = width * 0.5;
    let hd = depth * 0.5;
    // Bottom quad, back vertical face, sloped top, two sides.
    let v0 = Vec3::new(-hw, 0.0, -hd);
    let v1 = Vec3::new(hw, 0.0, -hd);
    let v2 = Vec3::new(hw, 0.0, hd);
    let v3 = Vec3::new(-hw, 0.0, hd);
    let v4 = Vec3::new(-hw, height, hd);
    let v5 = Vec3::new(hw, height, hd);
    let v6 = Vec3::new(-hw, height, -hd);
    let v7 = Vec3::new(hw, height, -hd);

    mesh.add_quad(v0, v1, v2, v3); // floor
    mesh.add_quad(v0, v3, v4, v6); // left side
    mesh.add_quad(v1, v7, v5, v2); // right side
    mesh.add_quad(v3, v2, v5, v4); // slope
    mesh.add_quad(v0, v6, v7, v1); // back wall
    mesh
}

/// Applies position / Y rotation / scale to mesh vertices (matches `Mesh::transform`).
pub fn transform_collider_mesh(
    mesh: &Mesh,
    position: Vec3,
    rotation_y: f32,
    scale: Vec3,
) -> Mesh {
    mesh.transform(position, rotation_y, scale)
}

/// Voxelizes `mesh` into a dense signed distance field.
pub fn build_mesh_sdf(mesh: &Mesh, resolution: u32) -> MeshSdf {
    let res = resolution.clamp(8, 96) as usize;
    let (bounds_min, bounds_max) = mesh_bounds(mesh);
    let pad = (bounds_max.sub(bounds_min)).length() * 0.08 + 0.05;
    let bounds_min = bounds_min.sub(Vec3::new(pad, pad, pad));
    let bounds_max = bounds_max.add(Vec3::new(pad, pad, pad));

    let nx = res;
    let ny = res;
    let nz = res;
    let count = nx * ny * nz;
    let mut values = vec![0.0; count];
    let vs = Vec3::new(
        (bounds_max.x - bounds_min.x) / nx as f32,
        (bounds_max.y - bounds_min.y) / ny as f32,
        (bounds_max.z - bounds_min.z) / nz as f32,
    );

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let world = Vec3::new(
                    bounds_min.x + (i as f32 + 0.5) * vs.x,
                    bounds_min.y + (j as f32 + 0.5) * vs.y,
                    bounds_min.z + (k as f32 + 0.5) * vs.z,
                );
                let unsigned = mesh_unsigned_distance(world, mesh);
                let inside = point_inside_mesh(world, mesh);
                let signed = if inside { -unsigned } else { unsigned };
                values[i + nx * (j + ny * k)] = signed;
            }
        }
    }

    MeshSdf {
        nx,
        ny,
        nz,
        bounds_min,
        bounds_max,
        values,
        display_mesh: mesh.clone(),
    }
}

fn mesh_bounds(mesh: &Mesh) -> (Vec3, Vec3) {
    if mesh.positions.is_empty() {
        return (Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5));
    }
    let mut mn = mesh.positions[0];
    let mut mx = mesh.positions[0];
    for p in &mesh.positions {
        mn.x = mn.x.min(p.x);
        mn.y = mn.y.min(p.y);
        mn.z = mn.z.min(p.z);
        mx.x = mx.x.max(p.x);
        mx.y = mx.y.max(p.y);
        mx.z = mx.z.max(p.z);
    }
    (mn, mx)
}

fn mesh_unsigned_distance(p: Vec3, mesh: &Mesh) -> f32 {
    let mut best = f32::MAX;
    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let a = mesh.positions[tri[0] as usize];
        let b = mesh.positions[tri[1] as usize];
        let c = mesh.positions[tri[2] as usize];
        best = best.min(point_triangle_distance(p, a, b, c));
    }
    best
}

/// Ray parity with deduplicated hit distances (handles quads split into two triangles).
fn point_inside_mesh(p: Vec3, mesh: &Mesh) -> bool {
    let dirs = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ];
    let mut inside_votes = 0u32;
    for dir in dirs {
        if ray_crossing_count(p, dir, mesh) % 2 == 1 {
            inside_votes += 1;
        }
    }
    inside_votes >= 2
}

fn ray_crossing_count(origin: Vec3, dir: Vec3, mesh: &Mesh) -> u32 {
    let mut ts = Vec::new();
    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let a = mesh.positions[tri[0] as usize];
        let b = mesh.positions[tri[1] as usize];
        let c = mesh.positions[tri[2] as usize];
        if let Some(t) = ray_triangle_distance(origin, dir, a, b, c) {
            ts.push(t);
        }
    }
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut unique = 0u32;
    let mut last = -1.0f32;
    for t in ts {
        if (t - last).abs() > 1e-4 {
            unique += 1;
            last = t;
        }
    }
    unique
}

fn ray_triangle_distance(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let ab = b.sub(a);
    let ac = c.sub(a);
    let pvec = dir.cross(ac);
    let det = ab.dot(pvec);
    if det.abs() < 1e-8 {
        return None;
    }
    let inv = 1.0 / det;
    let tvec = origin.sub(a);
    let u = tvec.dot(pvec) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qvec = tvec.cross(ab);
    let v = dir.dot(qvec) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = ac.dot(qvec) * inv;
    if t > 1e-5 {
        Some(t)
    } else {
        None
    }
}

fn point_triangle_distance(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f32 {
    let ab = b.sub(a);
    let ac = c.sub(a);
    let ap = p.sub(a);
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return ap.length();
    }

    let bp = p.sub(b);
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return bp.length();
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return p.sub(a.add(ab.scale(v))).length();
    }

    let cp = p.sub(c);
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return cp.length();
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return p.sub(a.add(ac.scale(w))).length();
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return p.sub(b.add(c.sub(b).scale(w))).length();
    }

    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    p.sub(a.add(ab.scale(v)).add(ac.scale(w))).length()
}

fn sample_trilinear(field: &[f32], nx: usize, ny: usize, nz: usize, x: f32, y: f32, z: f32) -> f32 {
    let x = x.clamp(0.0, nx as f32 - 1.001);
    let y = y.clamp(0.0, ny as f32 - 1.001);
    let z = z.clamp(0.0, nz as f32 - 1.001);

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let z0 = z.floor() as usize;
    let x1 = (x0 + 1).min(nx - 1);
    let y1 = (y0 + 1).min(ny - 1);
    let z1 = (z0 + 1).min(nz - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let tz = z - z0 as f32;

    let idx = |i: usize, j: usize, k: usize| i + nx * (j + ny * k);

    let c000 = field[idx(x0, y0, z0)];
    let c100 = field[idx(x1, y0, z0)];
    let c010 = field[idx(x0, y1, z0)];
    let c110 = field[idx(x1, y1, z0)];
    let c001 = field[idx(x0, y0, z1)];
    let c101 = field[idx(x1, y0, z1)];
    let c011 = field[idx(x0, y1, z1)];
    let c111 = field[idx(x1, y1, z1)];

    let c00 = c000 * (1.0 - tx) + c100 * tx;
    let c10 = c010 * (1.0 - tx) + c110 * tx;
    let c01 = c001 * (1.0 - tx) + c101 * tx;
    let c11 = c011 * (1.0 - tx) + c111 * tx;
    let c0 = c00 * (1.0 - ty) + c10 * ty;
    let c1 = c01 * (1.0 - ty) + c11 * ty;
    c0 * (1.0 - tz) + c1 * tz
}

/// Triangle-edge wireframe segments for viewport overlay.
pub fn mesh_wireframe_segments(mesh: &Mesh) -> Vec<[f32; 3]> {
    let mut lines = Vec::new();
    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let pts = [
            mesh.positions[tri[0] as usize],
            mesh.positions[tri[1] as usize],
            mesh.positions[tri[2] as usize],
        ];
        for edge in [(0, 1), (1, 2), (2, 0)] {
            let a = pts[edge.0];
            let b = pts[edge.1];
            lines.push([a.x, a.y, a.z]);
            lines.push([b.x, b.y, b.z]);
        }
    }
    lines
}

trait Vec3Ext {
    fn dot(self, other: Vec3) -> f32;
    fn cross(self, other: Vec3) -> Vec3;
}

impl Vec3Ext for Vec3 {
    fn dot(self, other: Vec3) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_box_sdf_negative_at_center() {
        let mesh = create_unit_box_collider();
        let sdf = build_mesh_sdf(&mesh, 24);
        let d = sdf.sample(Vec3::ZERO);
        assert!(d < 0.0, "center should be inside box, got {d}");
    }

    #[test]
    fn unit_box_sdf_positive_outside() {
        let mesh = create_unit_box_collider();
        let sdf = build_mesh_sdf(&mesh, 24);
        let d = sdf.sample(Vec3::new(2.0, 0.0, 0.0));
        assert!(d > 0.0, "far point should be outside, got {d}");
    }

    #[test]
    fn sphere_sdf_sign_at_center_and_outside() {
        let mesh = create_sphere_mesh(1.0, 16, 12);
        let sdf = build_mesh_sdf(&mesh, 28);
        assert!(sdf.sample(Vec3::ZERO) < 0.0);
        assert!(sdf.sample(Vec3::new(3.0, 0.0, 0.0)) > 0.0);
    }

    #[test]
    fn sphere_sdf_distance_near_surface() {
        let mesh = create_sphere_mesh(1.0, 20, 14);
        let sdf = build_mesh_sdf(&mesh, 32);
        let d = sdf.sample(Vec3::new(1.05, 0.0, 0.0)).abs();
        assert!(d < 0.15, "surface distance should be small, got {d}");
    }

    #[test]
    fn torus_mesh_has_triangles() {
        let mesh = create_torus_mesh(0.6, 0.15, 16, 8);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn ramp_mesh_has_triangles() {
        let mesh = create_ramp_mesh(2.0, 0.5, 2.0);
        assert!(mesh.triangle_count() >= 4);
    }
}
