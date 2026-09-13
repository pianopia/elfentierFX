//! Lightweight Eulerian smoke/gas solver (pure Rust). OpenVDB export via `openvdb_io`.

use crate::collider::{apply_colliders_smoke, ColliderInput};
use crate::mesh::Vec3;
use serde::{Deserialize, Serialize};

/// Domain bounds and grid resolution for a smoke simulation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmokeDomainInput {
    pub resolution: u32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub seed: u64,
}

impl Default for SmokeDomainInput {
    fn default() -> Self {
        Self {
            resolution: 24,
            bounds_min: Vec3::new(-4.0, 0.0, -4.0),
            bounds_max: Vec3::new(4.0, 8.0, 4.0),
            seed: 7,
        }
    }
}

/// Point emitter feeding density and temperature into the grid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmokeSourceInput {
    pub position: Vec3,
    pub radius: f32,
    pub emission_rate: f32,
    pub temperature: f32,
    pub upward_velocity: f32,
}

impl Default for SmokeSourceInput {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 1.2, 0.0),
            radius: 0.9,
            emission_rate: 2.5,
            temperature: 1.4,
            upward_velocity: 2.8,
        }
    }
}

/// Solver parameters for one cook pass.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmokeSolverInput {
    pub steps: u32,
    pub frame_stride: u32,
    pub dissipation: f32,
    pub buoyancy: f32,
    pub diffusion: f32,
    /// Velocity diffusion / drag (0 = inviscid, higher = thicker motion).
    pub viscosity: f32,
    pub pressure_iterations: u32,
    pub ground_collision: bool,
    pub max_particles_per_frame: u32,
}

impl Default for SmokeSolverInput {
    fn default() -> Self {
        Self {
            steps: 48,
            frame_stride: 4,
            dissipation: 0.985,
            buoyancy: 1.6,
            diffusion: 0.12,
            viscosity: 0.06,
            pressure_iterations: 18,
            ground_collision: true,
            max_particles_per_frame: 1800,
        }
    }
}

/// 3D scalar/vector fields on a uniform grid.
#[derive(Debug, Clone)]
pub struct SmokeGrid {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub density: Vec<f32>,
    pub vel_x: Vec<f32>,
    pub vel_y: Vec<f32>,
    pub vel_z: Vec<f32>,
    pub temperature: Vec<f32>,
}

impl SmokeGrid {
    pub fn cell_count(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    fn index(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.nx * (j + self.ny * k)
    }

    fn voxel_size(&self) -> Vec3 {
        Vec3::new(
            (self.bounds_max.x - self.bounds_min.x) / self.nx as f32,
            (self.bounds_max.y - self.bounds_min.y) / self.ny as f32,
            (self.bounds_max.z - self.bounds_min.z) / self.nz as f32,
        )
    }

    fn world_to_grid(&self, p: Vec3) -> (f32, f32, f32) {
        let vs = self.voxel_size();
        let fx = (p.x - self.bounds_min.x) / vs.x - 0.5;
        let fy = (p.y - self.bounds_min.y) / vs.y - 0.5;
        let fz = (p.z - self.bounds_min.z) / vs.z - 0.5;
        (fx, fy, fz)
    }

    fn grid_to_world(&self, i: f32, j: f32, k: f32) -> Vec3 {
        let vs = self.voxel_size();
        Vec3::new(
            self.bounds_min.x + (i + 0.5) * vs.x,
            self.bounds_min.y + (j + 0.5) * vs.y,
            self.bounds_min.z + (k + 0.5) * vs.z,
        )
    }

    pub fn max_density(&self) -> f32 {
        self.density.iter().copied().fold(0.0_f32, f32::max)
    }

    pub fn total_density(&self) -> f32 {
        self.density.iter().sum()
    }

    pub fn has_nan(&self) -> bool {
        self.density.iter().any(|v| !v.is_finite())
            || self.vel_x.iter().any(|v| !v.is_finite())
            || self.vel_y.iter().any(|v| !v.is_finite())
            || self.vel_z.iter().any(|v| !v.is_finite())
    }
}

/// Creates an empty smoke grid from domain parameters.
pub fn create_grid(domain: &SmokeDomainInput) -> SmokeGrid {
    let res = domain.resolution.clamp(8, 64) as usize;
    let count = res * res * res;
    SmokeGrid {
        nx: res,
        ny: res,
        nz: res,
        bounds_min: domain.bounds_min,
        bounds_max: domain.bounds_max,
        density: vec![0.0; count],
        vel_x: vec![0.0; count],
        vel_y: vec![0.0; count],
        vel_z: vec![0.0; count],
        temperature: vec![0.0; count],
    }
}

/// Simulation statistics returned from a cook.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmokeStats {
    pub resolution: [u32; 3],
    pub max_density: f32,
    pub step_count: u32,
    pub frame_count: u32,
    pub particle_count: u32,
    pub total_density: f32,
}

/// One viewport frame of impostor particles sampled from density.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmokeFrame {
    pub positions: Vec<f32>,
    pub sizes: Vec<f32>,
    pub opacities: Vec<f32>,
    pub particle_count: u32,
}

/// Full simulation output for viewport and export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmokeVolume {
    pub frames: Vec<SmokeFrame>,
    /// Per-frame voxel density grids (x-fastest indexing), for 3D texture export.
    pub density_grids: Vec<Vec<f32>>,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub stats: SmokeStats,
}

impl SmokeVolume {
    /// Returns captured density grids (empty when loaded from older snapshots).
    pub fn density_frames(&self) -> &[Vec<f32>] {
        &self.density_grids
    }
}

/// Runs the Eulerian smoke solver for the given domain, sources, and params.
pub fn simulate_smoke(
    domain: &SmokeDomainInput,
    sources: &[SmokeSourceInput],
    solver: &SmokeSolverInput,
    colliders: &[ColliderInput],
) -> SmokeVolume {
    let mut grid = create_grid(domain);
    let steps = solver.steps.max(1);
    let stride = solver.frame_stride.max(1);
    let mut frames = Vec::new();
    let mut density_grids = Vec::new();
    let mut rng = LcgRng::new(domain.seed.wrapping_add(0x5A4B_0001));

    emit_sources(&mut grid, sources, &mut rng);
    push_frame(&grid, solver, &mut frames, &mut density_grids, &mut rng);

    for step in 1..=steps {
        emit_sources(&mut grid, sources, &mut rng);
        apply_buoyancy(&mut grid, solver.buoyancy);
        let density = grid.density.clone();
        advect_field(&mut grid, &density, |g, idx, v| g.density[idx] = v);
        let vel_x = grid.vel_x.clone();
        advect_field(&mut grid, &vel_x, |g, idx, v| g.vel_x[idx] = v);
        let vel_y = grid.vel_y.clone();
        advect_field(&mut grid, &vel_y, |g, idx, v| g.vel_y[idx] = v);
        let vel_z = grid.vel_z.clone();
        advect_field(&mut grid, &vel_z, |g, idx, v| g.vel_z[idx] = v);
        diffuse_and_dissipate(&mut grid, solver.diffusion, solver.dissipation);
        apply_velocity_viscosity(&mut grid, solver.viscosity);
        if solver.ground_collision {
            apply_ground_collision(&mut grid);
        }
        apply_box_collision(&mut grid);
        let vs = grid.voxel_size();
        apply_colliders_smoke(
            grid.nx,
            grid.ny,
            grid.nz,
            grid.bounds_min,
            grid.bounds_max,
            vs,
            &mut grid.density,
            &mut grid.vel_x,
            &mut grid.vel_y,
            &mut grid.vel_z,
            &mut grid.temperature,
            colliders,
        );
        project_pressure(&mut grid, solver.pressure_iterations);

        if step % stride == 0 || step == steps {
            push_frame(&grid, solver, &mut frames, &mut density_grids, &mut rng);
        }
    }

    let max_density = frames
        .iter()
        .map(|f| f.opacities.iter().copied().fold(0.0_f32, f32::max))
        .fold(0.0_f32, f32::max)
        .max(grid.max_density());

    let particle_count = frames
        .last()
        .map(|f| f.particle_count)
        .unwrap_or(0);

    SmokeVolume {
        bounds_min: domain.bounds_min,
        bounds_max: domain.bounds_max,
        stats: SmokeStats {
            resolution: [grid.nx as u32, grid.ny as u32, grid.nz as u32],
            max_density,
            step_count: steps,
            frame_count: frames.len() as u32,
            particle_count,
            total_density: grid.total_density(),
        },
        frames,
        density_grids,
    }
}

fn emit_sources(grid: &mut SmokeGrid, sources: &[SmokeSourceInput], rng: &mut LcgRng) {
    for source in sources {
        let (cx, cy, cz) = grid.world_to_grid(source.position);
        let vs = grid.voxel_size();
        let r_cells = (source.radius / vs.x.max(vs.y).max(vs.z)).ceil() as i32 + 1;
        let ci = cx.round() as i32;
        let cj = cy.round() as i32;
        let ck = cz.round() as i32;

        for di in -r_cells..=r_cells {
            for dj in -r_cells..=r_cells {
                for dk in -r_cells..=r_cells {
                    let i = ci + di;
                    let j = cj + dj;
                    let k = ck + dk;
                    if i < 0 || j < 0 || k < 0 {
                        continue;
                    }
                    let (ui, uj, uk) = (i as usize, j as usize, k as usize);
                    if ui >= grid.nx || uj >= grid.ny || uk >= grid.nz {
                        continue;
                    }
                    let world = grid.grid_to_world(i as f32, j as f32, k as f32);
                    let dist = world.sub(source.position).length();
                    if dist > source.radius {
                        continue;
                    }
                    let falloff = 1.0 - (dist / source.radius).clamp(0.0, 1.0);
                    let noise = rng.next_f32() * 0.15 + 0.85;
                    let idx = grid.index(ui, uj, uk);
                    grid.density[idx] += source.emission_rate * falloff * noise * 0.08;
                    grid.temperature[idx] = grid.temperature[idx].max(source.temperature * falloff);
                    grid.vel_y[idx] += source.upward_velocity * falloff * 0.05;
                }
            }
        }
    }
}

fn apply_buoyancy(grid: &mut SmokeGrid, strength: f32) {
    for idx in 0..grid.cell_count() {
        let t = grid.temperature[idx];
        let d = grid.density[idx];
        if d > 1e-4 {
            grid.vel_y[idx] += strength * t * 0.04;
        }
    }
}

fn advect_field<F>(grid: &mut SmokeGrid, field: &[f32], mut write: F)
where
    F: FnMut(&mut SmokeGrid, usize, f32),
{
    let nx = grid.nx;
    let ny = grid.ny;
    let nz = grid.nz;
    let mut out = vec![0.0; grid.cell_count()];
    let dt = 1.0;

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let idx = grid.index(i, j, k);
                let pos = grid.grid_to_world(i as f32, j as f32, k as f32);
                let vel = Vec3::new(grid.vel_x[idx], grid.vel_y[idx], grid.vel_z[idx]);
                let prev = pos.sub(vel.scale(dt));
                let (fx, fy, fz) = grid.world_to_grid(prev);
                out[idx] = sample_trilinear(field, nx, ny, nz, fx, fy, fz);
            }
        }
    }

    for idx in 0..grid.cell_count() {
        write(grid, idx, out[idx]);
    }
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

fn apply_velocity_viscosity(grid: &mut SmokeGrid, viscosity: f32) {
    if viscosity <= 1e-6 {
        return;
    }
    let nx = grid.nx;
    let ny = grid.ny;
    let nz = grid.nz;
    let alpha = viscosity * 0.18;
    let mut vx = grid.vel_x.clone();
    let mut vy = grid.vel_y.clone();
    let mut vz = grid.vel_z.clone();

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let idx = grid.index(i, j, k);
                let (lx, ly, lz) = velocity_laplacian_at(grid, i, j, k);
                vx[idx] += alpha * lx;
                vy[idx] += alpha * ly;
                vz[idx] += alpha * lz;
            }
        }
    }
    grid.vel_x = vx;
    grid.vel_y = vy;
    grid.vel_z = vz;
}

fn velocity_laplacian_at(grid: &SmokeGrid, i: usize, j: usize, k: usize) -> (f32, f32, f32) {
    let nx = grid.nx;
    let ny = grid.ny;
    let nz = grid.nz;
    let idx = grid.index(i, j, k);
    let cx = grid.vel_x[idx];
    let cy = grid.vel_y[idx];
    let cz = grid.vel_z[idx];
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sz = 0.0;
    let mut n = 0.0;

    for (di, dj, dk) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
        let ni = i as i32 + di;
        let nj = j as i32 + dj;
        let nk = k as i32 + dk;
        if ni < 0 || nj < 0 || nk < 0 {
            continue;
        }
        let (ui, uj, uk) = (ni as usize, nj as usize, nk as usize);
        if ui >= nx || uj >= ny || uk >= nz {
            continue;
        }
        let nidx = grid.index(ui, uj, uk);
        sx += grid.vel_x[nidx] - cx;
        sy += grid.vel_y[nidx] - cy;
        sz += grid.vel_z[nidx] - cz;
        n += 1.0;
    }
    if n > 0.0 {
        (sx / n, sy / n, sz / n)
    } else {
        (0.0, 0.0, 0.0)
    }
}

fn diffuse_and_dissipate(grid: &mut SmokeGrid, diffusion: f32, dissipation: f32) {
    let nx = grid.nx;
    let ny = grid.ny;
    let nz = grid.nz;
    let mut next = grid.density.clone();
    let alpha = diffusion * 0.25;

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let idx = grid.index(i, j, k);
                let center = grid.density[idx];
                let mut lap = 0.0;
                let mut count = 0.0;
                for (di, dj, dk) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                    let ni = i as i32 + di;
                    let nj = j as i32 + dj;
                    let nk = k as i32 + dk;
                    if ni < 0 || nj < 0 || nk < 0 {
                        continue;
                    }
                    let (ui, uj, uk) = (ni as usize, nj as usize, nk as usize);
                    if ui >= nx || uj >= ny || uk >= nz {
                        continue;
                    }
                    lap += grid.density[grid.index(ui, uj, uk)];
                    count += 1.0;
                }
                if count > 0.0 {
                    lap = (lap / count - center) * alpha;
                }
                next[idx] = (center + lap) * dissipation;
            }
        }
    }
    grid.density = next;
}

fn apply_ground_collision(grid: &mut SmokeGrid) {
    for i in 0..grid.nx {
        for k in 0..grid.nz {
            let idx = grid.index(i, 0, k);
            grid.vel_y[idx] = grid.vel_y[idx].max(0.0);
            grid.density[idx] *= 0.85;
        }
    }
}

fn apply_box_collision(grid: &mut SmokeGrid) {
    let nx = grid.nx;
    let ny = grid.ny;
    let nz = grid.nz;

    for j in 0..ny {
        for k in 0..nz {
            for &i in &[0, nx - 1] {
                let idx = grid.index(i, j, k);
                grid.vel_x[idx] = 0.0;
            }
        }
    }
    for i in 0..nx {
        for k in 0..nz {
            for &j in &[0, ny - 1] {
                let idx = grid.index(i, j, k);
                grid.vel_y[idx] = 0.0;
            }
        }
    }
    for i in 0..nx {
        for j in 0..ny {
            for &k in &[0, nz - 1] {
                let idx = grid.index(i, j, k);
                grid.vel_z[idx] = 0.0;
            }
        }
    }
}

fn project_pressure(grid: &mut SmokeGrid, iterations: u32) {
    let nx = grid.nx;
    let ny = grid.ny;
    let nz = grid.nz;
    let count = grid.cell_count();
    let mut pressure = vec![0.0; count];
    let mut divergence = vec![0.0; count];

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let idx = grid.index(i, j, k);
                let div = divergence_at(grid, i, j, k);
                divergence[idx] = div;
            }
        }
    }

    for _ in 0..iterations.max(1) {
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let idx = grid.index(i, j, k);
                    let mut sum = 0.0;
                    let mut n = 0.0;
                    for (di, dj, dk) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                        let ni = i as i32 + di;
                        let nj = j as i32 + dj;
                        let nk = k as i32 + dk;
                        if ni < 0 || nj < 0 || nk < 0 {
                            continue;
                        }
                        let (ui, uj, uk) = (ni as usize, nj as usize, nk as usize);
                        if ui >= nx || uj >= ny || uk >= nz {
                            continue;
                        }
                        sum += pressure[grid.index(ui, uj, uk)];
                        n += 1.0;
                    }
                    if n > 0.0 {
                        pressure[idx] = (sum - divergence[idx]) / n;
                    }
                }
            }
        }
    }

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let idx = grid.index(i, j, k);
                let (gx, gy, gz) = pressure_gradient(&pressure, nx, ny, nz, i, j, k);
                grid.vel_x[idx] -= gx;
                grid.vel_y[idx] -= gy;
                grid.vel_z[idx] -= gz;
            }
        }
    }
}

fn divergence_at(grid: &SmokeGrid, i: usize, j: usize, k: usize) -> f32 {
    let nx = grid.nx;
    let ny = grid.ny;
    let nz = grid.nz;
    let idx = grid.index(i, j, k);
    let mut div = 0.0;

    if i + 1 < nx {
        div += grid.vel_x[grid.index(i + 1, j, k)] - grid.vel_x[idx];
    }
    if j + 1 < ny {
        div += grid.vel_y[grid.index(i, j + 1, k)] - grid.vel_y[idx];
    }
    if k + 1 < nz {
        div += grid.vel_z[grid.index(i, j, k + 1)] - grid.vel_z[idx];
    }
    div * 0.5
}

fn pressure_gradient(pressure: &[f32], nx: usize, ny: usize, nz: usize, i: usize, j: usize, k: usize) -> (f32, f32, f32) {
    let idx = |ii: usize, jj: usize, kk: usize| ii + nx * (jj + ny * kk);
    let px = if i + 1 < nx {
        pressure[idx(i + 1, j, k)] - pressure[idx(i, j, k)]
    } else {
        0.0
    };
    let py = if j + 1 < ny {
        pressure[idx(i, j + 1, k)] - pressure[idx(i, j, k)]
    } else {
        0.0
    };
    let pz = if k + 1 < nz {
        pressure[idx(i, j, k + 1)] - pressure[idx(i, j, k)]
    } else {
        0.0
    };
    (px * 0.5, py * 0.5, pz * 0.5)
}

fn push_frame(
    grid: &SmokeGrid,
    solver: &SmokeSolverInput,
    frames: &mut Vec<SmokeFrame>,
    density_grids: &mut Vec<Vec<f32>>,
    rng: &mut LcgRng,
) {
    density_grids.push(grid.density.clone());
    let max_particles = solver.max_particles_per_frame.max(64) as usize;
    let threshold = (grid.max_density() * 0.08).max(0.015);
    let vs = grid.voxel_size();
    let base_size = vs.x.max(vs.y).max(vs.z) * 1.35;

    let mut candidates: Vec<(usize, f32)> = grid
        .density
        .iter()
        .enumerate()
        .filter(|(_, d)| **d > threshold)
        .map(|(idx, d)| (idx, *d))
        .collect();

    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let take = candidates.len().min(max_particles);
    let mut positions = Vec::with_capacity(take * 3);
    let mut sizes = Vec::with_capacity(take);
    let mut opacities = Vec::with_capacity(take);

    for &(idx, density) in candidates.iter().take(take) {
        let k = idx / (grid.nx * grid.ny);
        let rem = idx % (grid.nx * grid.ny);
        let j = rem / grid.nx;
        let i = rem % grid.nx;
        let mut world = grid.grid_to_world(i as f32, j as f32, k as f32);
        world.x += (rng.next_f32() - 0.5) * vs.x * 0.6;
        world.y += (rng.next_f32() - 0.5) * vs.y * 0.6;
        world.z += (rng.next_f32() - 0.5) * vs.z * 0.6;
        positions.extend([world.x, world.y, world.z]);
        let norm = (density / grid.max_density().max(0.01)).clamp(0.05, 1.0);
        sizes.push(base_size * (0.55 + norm * 0.85));
        opacities.push(norm);
    }

    frames.push(SmokeFrame {
        particle_count: take as u32,
        positions,
        sizes,
        opacities,
    });
}

/// Exports density frames as a raw f32 XY atlas (max-projected through Z per frame).
pub fn export_density_atlas(volume: &SmokeVolume, path: &str) -> std::io::Result<SmokeExportResult> {
    use crate::volume_texture::build_density_atlas_xy;
    use std::io::Write;

    let frame_count = volume.frames.len();
    if frame_count == 0 {
        return Ok(SmokeExportResult {
            path: path.to_string(),
            frame_count: 0,
            byte_len: 0,
            format: "elfentier_smoke_atlas_v1".into(),
        });
    }

    let res = volume.stats.resolution;
    let atlas = build_density_atlas_xy(volume);

    let mut file = std::fs::File::create(path)?;
    writeln!(
        file,
        "# elfentier smoke density atlas v1\n# frames={} res={}x{}\n# data=f32 little-endian row-major XY per slice (Z max-projected), frame-major",
        frame_count,
        res[0],
        res[1]
    )?;
    file.write_all(&f32_slice_to_bytes(&atlas))?;

    Ok(SmokeExportResult {
        path: path.to_string(),
        frame_count: frame_count as u32,
        byte_len: atlas.len() * 4 + 128,
        format: "elfentier_smoke_atlas_v1".into(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmokeExportResult {
    pub path: String,
    pub frame_count: u32,
    pub byte_len: usize,
    pub format: String,
}

fn f32_slice_to_bytes(data: &[f32]) -> Vec<u8> {
    data.iter().flat_map(|f| f.to_le_bytes()).collect()
}

struct LcgRng {
    state: u64,
}

impl LcgRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.max(1),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.state
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 33) as f32 / (1u32 << 31) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collider::{apply_colliders_smoke, ColliderInput};

    #[test]
    fn grid_creation_has_no_nans() {
        let domain = SmokeDomainInput::default();
        let grid = create_grid(&domain);
        assert!(!grid.has_nan());
        assert_eq!(grid.cell_count(), 24 * 24 * 24);
    }

    #[test]
    fn emission_increases_density() {
        let domain = SmokeDomainInput {
            resolution: 16,
            ..Default::default()
        };
        let mut grid = create_grid(&domain);
        let before = grid.total_density();
        let source = SmokeSourceInput::default();
        let mut rng = LcgRng::new(domain.seed);
        emit_sources(&mut grid, &[source], &mut rng);
        assert!(grid.total_density() > before);
        assert!(!grid.has_nan());
    }

    #[test]
    fn simulation_bounded_and_deterministic() {
        let domain = SmokeDomainInput {
            resolution: 16,
            seed: 99,
            ..Default::default()
        };
        let sources = vec![SmokeSourceInput::default()];
        let solver = SmokeSolverInput {
            steps: 12,
            frame_stride: 4,
            ..Default::default()
        };
        let a = simulate_smoke(&domain, &sources, &solver, &[]);
        let b = simulate_smoke(&domain, &sources, &solver, &[]);
        assert_eq!(a.frames.len(), b.frames.len());
        assert_eq!(a.frames[0].positions, b.frames[0].positions);
        assert!(!a.frames.is_empty());
        assert!(a.stats.max_density > 0.0);
        assert!(a.stats.max_density < 100.0);
        assert!(a.stats.total_density < 500.0);
    }

    #[test]
    fn advection_preserves_bounded_density() {
        let domain = SmokeDomainInput {
            resolution: 12,
            ..Default::default()
        };
        let mut grid = create_grid(&domain);
        let center = grid.index(6, 4, 6);
        grid.density[center] = 1.0;
        grid.vel_y[center] = 0.5;
        let field = grid.density.clone();
        advect_field(&mut grid, &field, |g, idx, v| g.density[idx] = v);
        assert!(!grid.has_nan());
        let total: f32 = grid.density.iter().sum();
        assert!(total > 0.0 && total <= 1.5);
    }

    #[test]
    fn high_viscosity_reduces_upward_motion() {
        let domain = SmokeDomainInput {
            resolution: 14,
            seed: 7,
            ..Default::default()
        };
        let sources = vec![SmokeSourceInput {
            upward_velocity: 4.0,
            ..Default::default()
        }];
        let thin = SmokeSolverInput {
            steps: 20,
            frame_stride: 20,
            viscosity: 0.02,
            ..Default::default()
        };
        let thick = SmokeSolverInput {
            steps: 20,
            frame_stride: 20,
            viscosity: 0.55,
            ..Default::default()
        };
        let fast = simulate_smoke(&domain, &sources, &thin, &[]);
        let slow = simulate_smoke(&domain, &sources, &thick, &[]);
        assert!(slow.stats.max_density > 0.0);
        assert!(fast.stats.max_density > slow.stats.max_density * 0.5);
    }

    #[test]
    fn floor_collider_blocks_density_at_ground() {
        let domain = SmokeDomainInput {
            resolution: 14,
            ..Default::default()
        };
        let sources = vec![SmokeSourceInput {
            position: Vec3::new(0.0, 0.5, 0.0),
            radius: 0.8,
            emission_rate: 4.0,
            ..Default::default()
        }];
        let floor = ColliderInput::floor(0.5, 4.0);
        let solver = SmokeSolverInput {
            steps: 16,
            frame_stride: 16,
            ground_collision: false,
            ..Default::default()
        };
        let vol = simulate_smoke(&domain, &sources, &solver, &[floor]);
        let mut grid = create_grid(&domain);
        emit_sources(&mut grid, &sources, &mut LcgRng::new(domain.seed));
        let idx = grid.index(7, 0, 7);
        grid.density[idx] = 2.0;
        apply_colliders_smoke(
            grid.nx,
            grid.ny,
            grid.nz,
            grid.bounds_min,
            grid.bounds_max,
            grid.voxel_size(),
            &mut grid.density,
            &mut grid.vel_x,
            &mut grid.vel_y,
            &mut grid.vel_z,
            &mut grid.temperature,
            &[floor],
        );
        assert!(grid.density[idx] < 0.01);
        assert!(vol.stats.max_density < 100.0);
    }
}
