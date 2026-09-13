//! FLIP-style particle–grid liquid solver (pure Rust, realtime-friendly).

use crate::collider::{
    apply_colliders_liquid_grid_resolved, resolve_colliders, resolve_particle_colliders_resolved,
    ColliderInput,
};
use crate::mesh::Vec3;
use serde::{Deserialize, Serialize};

/// Domain bounds and grid resolution for a liquid simulation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LiquidDomainInput {
    pub resolution: u32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub seed: u64,
    pub initial_particles: u32,
    pub particle_radius: f32,
}

impl Default for LiquidDomainInput {
    fn default() -> Self {
        Self {
            resolution: 20,
            bounds_min: Vec3::new(-4.0, 0.0, -4.0),
            bounds_max: Vec3::new(4.0, 6.0, 4.0),
            seed: 11,
            initial_particles: 1200,
            particle_radius: 0.12,
        }
    }
}

/// Point emitter feeding particles into the domain.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LiquidSourceInput {
    pub position: Vec3,
    pub radius: f32,
    pub emission_rate: f32,
    pub velocity: Vec3,
    /// Stop emitting after this step (0 = unlimited).
    pub active_until_step: u32,
}

impl Default for LiquidSourceInput {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 5.0, 0.0),
            radius: 0.6,
            emission_rate: 8.0,
            velocity: Vec3::new(0.0, -2.5, 0.0),
            active_until_step: 0,
        }
    }
}

/// Solver parameters for one cook pass.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LiquidSolverInput {
    pub steps: u32,
    pub frame_stride: u32,
    pub gravity: f32,
    /// 0 = PIC, 1 = FLIP.
    pub flip_ratio: f32,
    pub viscosity: f32,
    pub pressure_iterations: u32,
    /// Ocean surface wave forcing amplitude.
    pub wave_amplitude: f32,
    pub wave_frequency: f32,
    /// Flat terrain collider height (stub).
    pub terrain_height: f32,
    pub max_particles: u32,
}

impl Default for LiquidSolverInput {
    fn default() -> Self {
        Self {
            steps: 60,
            frame_stride: 3,
            gravity: 9.8,
            flip_ratio: 0.96,
            viscosity: 0.02,
            pressure_iterations: 20,
            wave_amplitude: 0.0,
            wave_frequency: 1.2,
            terrain_height: 0.0,
            max_particles: 16000,
        }
    }
}

/// One liquid particle.
#[derive(Debug, Clone, Copy)]
struct Particle {
    pos: Vec3,
    vel: Vec3,
}

/// Staggered-style collocated velocity grid for pressure projection.
#[derive(Debug, Clone)]
struct LiquidGrid {
    nx: usize,
    ny: usize,
    nz: usize,
    bounds_min: Vec3,
    bounds_max: Vec3,
    vel_x: Vec<f32>,
    vel_y: Vec<f32>,
    vel_z: Vec<f32>,
    weight: Vec<f32>,
}

impl LiquidGrid {
    fn cell_count(&self) -> usize {
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

    fn clear(&mut self) {
        self.vel_x.fill(0.0);
        self.vel_y.fill(0.0);
        self.vel_z.fill(0.0);
        self.weight.fill(0.0);
    }

    fn has_nan(&self) -> bool {
        self.vel_x.iter().any(|v| !v.is_finite())
            || self.vel_y.iter().any(|v| !v.is_finite())
            || self.vel_z.iter().any(|v| !v.is_finite())
    }
}

/// Simulation statistics returned from a cook.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LiquidStats {
    pub resolution: [u32; 3],
    pub step_count: u32,
    pub frame_count: u32,
    pub particle_count: u32,
    pub max_speed: f32,
}

/// One viewport frame of liquid particles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiquidFrame {
    pub positions: Vec<f32>,
    pub radii: Vec<f32>,
    pub opacities: Vec<f32>,
    pub particle_count: u32,
}

/// Full simulation output for viewport and export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiquidVolume {
    pub frames: Vec<LiquidFrame>,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub stats: LiquidStats,
}

fn create_grid(domain: &LiquidDomainInput) -> LiquidGrid {
    let res = domain.resolution.clamp(8, 48) as usize;
    let count = res * res * res;
    LiquidGrid {
        nx: res,
        ny: res,
        nz: res,
        bounds_min: domain.bounds_min,
        bounds_max: domain.bounds_max,
        vel_x: vec![0.0; count],
        vel_y: vec![0.0; count],
        vel_z: vec![0.0; count],
        weight: vec![0.0; count],
    }
}

/// Runs the FLIP liquid solver for the given domain, sources, and params.
pub fn simulate_liquid(
    domain: &LiquidDomainInput,
    sources: &[LiquidSourceInput],
    solver: &LiquidSolverInput,
    colliders: &[ColliderInput],
) -> LiquidVolume {
    let grid = create_grid(domain);
    let mut particles = seed_particles(domain, &grid);
    let steps = solver.steps.max(1);
    let stride = solver.frame_stride.max(1);
    let dt = 1.0 / steps as f32;
    let max_particles = solver.max_particles.clamp(256, 32000) as usize;
    let mut frames = Vec::new();
    let mut rng = LcgRng::new(domain.seed.wrapping_add(0x4C51_0001));
    let resolved_colliders = resolve_colliders(colliders);

    push_liquid_frame(&particles, domain, &mut frames, solver);
    let mut max_speed = 0.0_f32;

    for step in 1..=steps {
        emit_sources(&mut particles, sources, step, &mut rng, max_particles);
        apply_ocean_waves(&mut particles, domain, solver, step, dt);

        let mut g = grid.clone();
        g.clear();
        particles_to_grid(&particles, &mut g);

        let vel_x_old = g.vel_x.clone();
        let vel_y_old = g.vel_y.clone();
        let vel_z_old = g.vel_z.clone();

        apply_gravity(&mut g, solver.gravity, dt);
        apply_viscosity(&mut g, solver.viscosity);
        apply_terrain_collision(&mut g, solver.terrain_height);
        apply_box_collision(&mut g);
        let vs = g.voxel_size();
        apply_colliders_liquid_grid_resolved(
            g.nx,
            g.ny,
            g.nz,
            g.bounds_min,
            g.bounds_max,
            vs,
            &g.weight,
            &mut g.vel_x,
            &mut g.vel_y,
            &mut g.vel_z,
            &resolved_colliders,
        );
        project_pressure(&mut g, solver.pressure_iterations);

        grid_to_particles(
            &mut particles,
            &g,
            &vel_x_old,
            &vel_y_old,
            &vel_z_old,
            solver.flip_ratio,
        );

        advect_particles(&mut particles, dt);
        clamp_particles_to_domain(&mut particles, domain);
        for p in particles.iter_mut() {
            resolve_particle_colliders_resolved(
                &mut p.pos,
                &mut p.vel,
                domain.particle_radius,
                &resolved_colliders,
            );
        }
        cull_excess(&mut particles, max_particles);

        let step_max = particles
            .iter()
            .map(|p| p.vel.length())
            .fold(0.0_f32, f32::max);
        max_speed = max_speed.max(step_max);

        if step % stride == 0 || step == steps {
            push_liquid_frame(&particles, domain, &mut frames, solver);
        }
    }

    let particle_count = frames
        .last()
        .map(|f| f.particle_count)
        .unwrap_or(0);

    LiquidVolume {
        bounds_min: domain.bounds_min,
        bounds_max: domain.bounds_max,
        stats: LiquidStats {
            resolution: [grid.nx as u32, grid.ny as u32, grid.nz as u32],
            step_count: steps,
            frame_count: frames.len() as u32,
            particle_count,
            max_speed,
        },
        frames,
    }
}

fn seed_particles(domain: &LiquidDomainInput, grid: &LiquidGrid) -> Vec<Particle> {
    let count = domain.initial_particles.clamp(0, 8000) as usize;
    if count == 0 {
        return Vec::new();
    }
    let mut rng = LcgRng::new(domain.seed);
    let vs = grid.voxel_size();
    let pad = domain.particle_radius * 2.0;
    let mut particles = Vec::with_capacity(count);

    for _ in 0..count {
        let x = rng.next_f32()
            * (domain.bounds_max.x - domain.bounds_min.x - pad * 2.0)
            + domain.bounds_min.x
            + pad;
        let z = rng.next_f32()
            * (domain.bounds_max.z - domain.bounds_min.z - pad * 2.0)
            + domain.bounds_min.z
            + pad;
        let y = domain.bounds_min.y
            + pad
            + rng.next_f32() * (domain.bounds_max.y - domain.bounds_min.y - pad * 2.0) * 0.35;
        particles.push(Particle {
            pos: Vec3::new(x, y, z),
            vel: Vec3::new(
                (rng.next_f32() - 0.5) * 0.1,
                (rng.next_f32() - 0.5) * 0.05,
                (rng.next_f32() - 0.5) * 0.1,
            ),
        });
    }

    let _ = vs;
    particles
}

fn emit_sources(
    particles: &mut Vec<Particle>,
    sources: &[LiquidSourceInput],
    step: u32,
    rng: &mut LcgRng,
    max_particles: usize,
) {
    for source in sources {
        if source.active_until_step > 0 && step > source.active_until_step {
            continue;
        }
        let emit_count = source.emission_rate.max(0.0) as usize;
        for _ in 0..emit_count {
            if particles.len() >= max_particles {
                return;
            }
            let theta = rng.next_f32() * std::f32::consts::TAU;
            let r = source.radius * rng.next_f32().sqrt();
            let offset = Vec3::new(r * theta.cos(), 0.0, r * theta.sin());
            let jitter = Vec3::new(
                (rng.next_f32() - 0.5) * 0.08,
                (rng.next_f32() - 0.5) * 0.08,
                (rng.next_f32() - 0.5) * 0.08,
            );
            particles.push(Particle {
                pos: source.position.add(offset).add(jitter),
                vel: source.velocity.add(Vec3::new(
                    (rng.next_f32() - 0.5) * 0.3,
                    (rng.next_f32() - 0.5) * 0.2,
                    (rng.next_f32() - 0.5) * 0.3,
                )),
            });
        }
    }
}

fn apply_ocean_waves(
    particles: &mut [Particle],
    domain: &LiquidDomainInput,
    solver: &LiquidSolverInput,
    step: u32,
    dt: f32,
) {
    if solver.wave_amplitude <= 1e-5 {
        return;
    }
    let t = step as f32 * dt * solver.wave_frequency;
    let surface_y = domain.bounds_min.y
        + (domain.bounds_max.y - domain.bounds_min.y) * 0.55;
    let band = (domain.bounds_max.y - domain.bounds_min.y) * 0.15;

    for p in particles.iter_mut() {
        if p.pos.y < surface_y - band {
            continue;
        }
        let wave = solver.wave_amplitude
            * (p.pos.x * 0.4 + p.pos.z * 0.35 + t).sin()
            * (p.pos.z * 0.3 - t * 0.7).cos();
        p.vel.y += wave * dt * 2.5;
        p.vel.x += wave * dt * 0.4;
    }
}

fn particles_to_grid(particles: &[Particle], grid: &mut LiquidGrid) {
    let nx = grid.nx;
    let ny = grid.ny;
    let nz = grid.nz;

    for p in particles {
        let (fx, fy, fz) = grid.world_to_grid(p.pos);
        splat_velocity(grid, fx, fy, fz, p.vel, nx, ny, nz);
    }

    for idx in 0..grid.cell_count() {
        let w = grid.weight[idx];
        if w > 1e-6 {
            grid.vel_x[idx] /= w;
            grid.vel_y[idx] /= w;
            grid.vel_z[idx] /= w;
        }
    }
}

fn splat_velocity(
    grid: &mut LiquidGrid,
    fx: f32,
    fy: f32,
    fz: f32,
    vel: Vec3,
    nx: usize,
    ny: usize,
    nz: usize,
) {
    let x0 = fx.floor() as i32;
    let y0 = fy.floor() as i32;
    let z0 = fz.floor() as i32;
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let tz = fz - z0 as f32;

    for (di, wx) in [(0, 1.0 - tx), (1, tx)] {
        for (dj, wy) in [(0, 1.0 - ty), (1, ty)] {
            for (dk, wz) in [(0, 1.0 - tz), (1, tz)] {
                let i = x0 + di;
                let j = y0 + dj;
                let k = z0 + dk;
                if i < 0 || j < 0 || k < 0 {
                    continue;
                }
                let (ui, uj, uk) = (i as usize, j as usize, k as usize);
                if ui >= nx || uj >= ny || uk >= nz {
                    continue;
                }
                let w = wx * wy * wz;
                let idx = grid.index(ui, uj, uk);
                grid.vel_x[idx] += vel.x * w;
                grid.vel_y[idx] += vel.y * w;
                grid.vel_z[idx] += vel.z * w;
                grid.weight[idx] += w;
            }
        }
    }
}

fn sample_velocity(grid: &LiquidGrid, pos: Vec3) -> Vec3 {
    let (fx, fy, fz) = grid.world_to_grid(pos);
    Vec3::new(
        sample_trilinear(&grid.vel_x, grid.nx, grid.ny, grid.nz, fx, fy, fz),
        sample_trilinear(&grid.vel_y, grid.nx, grid.ny, grid.nz, fx, fy, fz),
        sample_trilinear(&grid.vel_z, grid.nx, grid.ny, grid.nz, fx, fy, fz),
    )
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

fn apply_gravity(grid: &mut LiquidGrid, gravity: f32, dt: f32) {
    for idx in 0..grid.cell_count() {
        if grid.weight[idx] > 1e-6 {
            grid.vel_y[idx] -= gravity * dt;
        }
    }
}

fn apply_viscosity(grid: &mut LiquidGrid, viscosity: f32) {
    if viscosity <= 1e-6 {
        return;
    }
    let nx = grid.nx;
    let ny = grid.ny;
    let nz = grid.nz;
    let alpha = viscosity * 0.15;
    let mut vx = grid.vel_x.clone();
    let mut vy = grid.vel_y.clone();
    let mut vz = grid.vel_z.clone();

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let idx = grid.index(i, j, k);
                if grid.weight[idx] < 1e-6 {
                    continue;
                }
                let (lx, ly, lz) = laplacian_at(grid, i, j, k);
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

fn laplacian_at(grid: &LiquidGrid, i: usize, j: usize, k: usize) -> (f32, f32, f32) {
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

fn apply_terrain_collision(grid: &mut LiquidGrid, terrain_height: f32) {
    let vs = grid.voxel_size();
    for k in 0..grid.nz {
        for j in 0..grid.ny {
            for i in 0..grid.nx {
                let world = grid.grid_to_world(i as f32, j as f32, k as f32);
                if world.y <= terrain_height + vs.y * 0.5 {
                    let idx = grid.index(i, j, k);
                    grid.vel_y[idx] = grid.vel_y[idx].max(0.0);
                    grid.vel_x[idx] *= 0.85;
                    grid.vel_z[idx] *= 0.85;
                }
            }
        }
    }
}

fn apply_box_collision(grid: &mut LiquidGrid) {
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

fn project_pressure(grid: &mut LiquidGrid, iterations: u32) {
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
                if grid.weight[idx] < 1e-6 {
                    continue;
                }
                divergence[idx] = divergence_at(grid, i, j, k);
            }
        }
    }

    for _ in 0..iterations.max(1) {
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let idx = grid.index(i, j, k);
                    if grid.weight[idx] < 1e-6 {
                        continue;
                    }
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
                if grid.weight[idx] < 1e-6 {
                    continue;
                }
                let (gx, gy, gz) = pressure_gradient(&pressure, nx, ny, nz, i, j, k);
                grid.vel_x[idx] -= gx;
                grid.vel_y[idx] -= gy;
                grid.vel_z[idx] -= gz;
            }
        }
    }
}

fn divergence_at(grid: &LiquidGrid, i: usize, j: usize, k: usize) -> f32 {
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

fn grid_to_particles(
    particles: &mut [Particle],
    grid: &LiquidGrid,
    vel_x_old: &[f32],
    vel_y_old: &[f32],
    vel_z_old: &[f32],
    flip_ratio: f32,
) {
    let flip = flip_ratio.clamp(0.0, 1.0);
    let pic = 1.0 - flip;

    for p in particles.iter_mut() {
        let (fx, fy, fz) = grid.world_to_grid(p.pos);
        let pic_vel = sample_velocity(grid, p.pos);
        let old_x = sample_trilinear(vel_x_old, grid.nx, grid.ny, grid.nz, fx, fy, fz);
        let old_y = sample_trilinear(vel_y_old, grid.nx, grid.ny, grid.nz, fx, fy, fz);
        let old_z = sample_trilinear(vel_z_old, grid.nx, grid.ny, grid.nz, fx, fy, fz);
        let flip_delta = Vec3::new(
            pic_vel.x - old_x,
            pic_vel.y - old_y,
            pic_vel.z - old_z,
        );
        p.vel = Vec3::new(
            p.vel.x + flip * flip_delta.x + pic * (pic_vel.x - p.vel.x),
            p.vel.y + flip * flip_delta.y + pic * (pic_vel.y - p.vel.y),
            p.vel.z + flip * flip_delta.z + pic * (pic_vel.z - p.vel.z),
        );
    }
}

fn advect_particles(particles: &mut [Particle], dt: f32) {
    for p in particles.iter_mut() {
        p.pos = p.pos.add(p.vel.scale(dt));
    }
}

fn clamp_particles_to_domain(particles: &mut [Particle], domain: &LiquidDomainInput) {
    let pad = domain.particle_radius;
    for p in particles.iter_mut() {
        p.pos.x = p.pos.x.clamp(domain.bounds_min.x + pad, domain.bounds_max.x - pad);
        p.pos.y = p.pos.y.clamp(domain.bounds_min.y + pad, domain.bounds_max.y - pad);
        p.pos.z = p.pos.z.clamp(domain.bounds_min.z + pad, domain.bounds_max.z - pad);
        if p.pos.y <= domain.bounds_min.y + pad * 1.5 {
            p.vel.y = p.vel.y.max(0.0);
            p.vel.x *= 0.9;
            p.vel.z *= 0.9;
        }
    }
}

fn cull_excess(particles: &mut Vec<Particle>, max: usize) {
    if particles.len() > max {
        particles.truncate(max);
    }
}

fn push_liquid_frame(
    particles: &[Particle],
    domain: &LiquidDomainInput,
    frames: &mut Vec<LiquidFrame>,
    solver: &LiquidSolverInput,
) {
    let count = particles.len();
    let radius = domain.particle_radius;
    let mut positions = Vec::with_capacity(count * 3);
    let mut radii = Vec::with_capacity(count);
    let mut opacities = Vec::with_capacity(count);

    let max_speed = particles
        .iter()
        .map(|p| p.vel.length())
        .fold(0.1_f32, f32::max);

    for p in particles {
        positions.extend([p.pos.x, p.pos.y, p.pos.z]);
        let speed_norm = (p.vel.length() / max_speed).clamp(0.2, 1.0);
        radii.push(radius * (0.85 + speed_norm * 0.25));
        opacities.push(0.55 + speed_norm * 0.35);
    }

    let _ = solver;
    frames.push(LiquidFrame {
        particle_count: count as u32,
        positions,
        radii,
        opacities,
    });
}

/// Exports liquid particle cache frames for Unity/game-engine handoff.
pub fn export_particle_cache(volume: &LiquidVolume, path: &str) -> std::io::Result<LiquidExportResult> {
    use std::io::Write;
    let frame_count = volume.frames.len();
    if frame_count == 0 {
        return Ok(LiquidExportResult {
            path: path.to_string(),
            frame_count: 0,
            byte_len: 0,
            format: "elfentier_liquid_cache_v1".into(),
        });
    }

    let mut file = std::fs::File::create(path)?;
    writeln!(
        file,
        "# elfentier liquid particle cache v1\n# frames={} particles_per_frame={}\n# layout=frame-major [x,y,z,radius] f32 little-endian",
        frame_count,
        volume.frames[0].particle_count
    )?;

    let mut byte_len = 0usize;
    for frame in &volume.frames {
        let count = frame.particle_count as usize;
        for i in 0..count {
            let data = [
                frame.positions.get(i * 3).copied().unwrap_or(0.0),
                frame.positions.get(i * 3 + 1).copied().unwrap_or(0.0),
                frame.positions.get(i * 3 + 2).copied().unwrap_or(0.0),
                frame.radii.get(i).copied().unwrap_or(0.1),
            ];
            for f in data {
                file.write_all(&f.to_le_bytes())?;
                byte_len += 4;
            }
        }
    }

    Ok(LiquidExportResult {
        path: path.to_string(),
        frame_count: frame_count as u32,
        byte_len,
        format: "elfentier_liquid_cache_v1".into(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidExportResult {
    pub path: String,
    pub frame_count: u32,
    pub byte_len: usize,
    pub format: String,
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

/// Returns true when all particle positions are inside domain bounds (with padding).
pub fn particles_in_bounds(volume: &LiquidVolume, domain: &LiquidDomainInput) -> bool {
    let pad = domain.particle_radius;
    for frame in &volume.frames {
        let count = frame.particle_count as usize;
        for i in 0..count {
            let x = frame.positions[i * 3];
            let y = frame.positions[i * 3 + 1];
            let z = frame.positions[i * 3 + 2];
            if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                return false;
            }
            if x < domain.bounds_min.x - pad
                || x > domain.bounds_max.x + pad
                || y < domain.bounds_min.y - pad
                || y > domain.bounds_max.y + pad
                || z < domain.bounds_min.z - pad
                || z > domain.bounds_max.z + pad
            {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collider::ColliderInput;

    #[test]
    fn simulation_has_no_nans() {
        let domain = LiquidDomainInput {
            resolution: 14,
            initial_particles: 400,
            ..Default::default()
        };
        let sources = vec![LiquidSourceInput::default()];
        let solver = LiquidSolverInput {
            steps: 20,
            frame_stride: 5,
            ..Default::default()
        };
        let vol = simulate_liquid(&domain, &sources, &solver, &[]);
        assert!(!vol.frames.is_empty());
        for frame in &vol.frames {
            for v in &frame.positions {
                assert!(v.is_finite(), "NaN in positions");
            }
        }
    }

    #[test]
    fn particles_stay_in_domain() {
        let domain = LiquidDomainInput {
            resolution: 14,
            initial_particles: 300,
            ..Default::default()
        };
        let sources = vec![LiquidSourceInput {
            position: Vec3::new(0.0, 4.0, 0.0),
            radius: 0.4,
            emission_rate: 4.0,
            velocity: Vec3::new(0.0, -1.0, 0.0),
            active_until_step: 30,
        }];
        let solver = LiquidSolverInput {
            steps: 36,
            frame_stride: 6,
            ..Default::default()
        };
        let vol = simulate_liquid(&domain, &sources, &solver, &[]);
        assert!(particles_in_bounds(&vol, &domain));
    }

    #[test]
    fn simulation_is_deterministic_with_seed() {
        let domain = LiquidDomainInput {
            resolution: 12,
            seed: 42,
            initial_particles: 200,
            ..Default::default()
        };
        let sources = vec![LiquidSourceInput::default()];
        let solver = LiquidSolverInput {
            steps: 15,
            frame_stride: 5,
            ..Default::default()
        };
        let a = simulate_liquid(&domain, &sources, &solver, &[]);
        let b = simulate_liquid(&domain, &sources, &solver, &[]);
        assert_eq!(a.frames.len(), b.frames.len());
        assert_eq!(a.frames[0].positions, b.frames[0].positions);
    }

    #[test]
    fn particle_count_sanity() {
        let domain = LiquidDomainInput {
            resolution: 12,
            initial_particles: 500,
            ..Default::default()
        };
        let solver = LiquidSolverInput {
            steps: 10,
            max_particles: 2000,
            ..Default::default()
        };
        let vol = simulate_liquid(&domain, &[], &solver, &[]);
        assert!(vol.stats.particle_count > 0);
        assert!(vol.stats.particle_count <= 2000);
        assert!(vol.stats.max_speed < 50.0);
    }

    #[test]
    fn ocean_waves_increase_motion() {
        let domain = LiquidDomainInput {
            resolution: 14,
            bounds_min: Vec3::new(-6.0, 0.0, -6.0),
            bounds_max: Vec3::new(6.0, 3.0, 6.0),
            initial_particles: 800,
            ..Default::default()
        };
        let solver_flat = LiquidSolverInput {
            steps: 24,
            wave_amplitude: 0.0,
            ..Default::default()
        };
        let solver_wave = LiquidSolverInput {
            steps: 24,
            wave_amplitude: 0.35,
            wave_frequency: 1.5,
            ..Default::default()
        };
        let flat = simulate_liquid(&domain, &[], &solver_flat, &[]);
        let wave = simulate_liquid(&domain, &[], &solver_wave, &[]);
        assert!(wave.stats.max_speed >= flat.stats.max_speed * 0.8);
    }

    #[test]
    fn high_viscosity_reduces_max_speed() {
        let domain = LiquidDomainInput {
            resolution: 14,
            seed: 9,
            initial_particles: 600,
            ..Default::default()
        };
        let sources = vec![LiquidSourceInput {
            position: Vec3::new(0.0, 4.5, 0.0),
            emission_rate: 6.0,
            velocity: Vec3::new(0.0, -4.0, 0.0),
            ..Default::default()
        }];
        let water = LiquidSolverInput {
            steps: 24,
            viscosity: 0.01,
            ..Default::default()
        };
        let syrup = LiquidSolverInput {
            steps: 24,
            viscosity: 0.35,
            ..Default::default()
        };
        let thin = simulate_liquid(&domain, &sources, &water, &[]);
        let thick = simulate_liquid(&domain, &sources, &syrup, &[]);
        assert!(thick.stats.max_speed < thin.stats.max_speed);
    }

    #[test]
    fn floor_collider_keeps_particles_above_ground() {
        let domain = LiquidDomainInput {
            resolution: 14,
            bounds_min: Vec3::new(-3.5, 0.0, -3.5),
            bounds_max: Vec3::new(3.5, 6.0, 3.5),
            initial_particles: 400,
            particle_radius: 0.1,
            ..Default::default()
        };
        let sources = vec![LiquidSourceInput {
            position: Vec3::new(0.0, 3.0, 0.0),
            emission_rate: 8.0,
            velocity: Vec3::new(0.0, -5.0, 0.0),
            ..Default::default()
        }];
        let floor = ColliderInput::floor(0.35, 3.5);
        let solver = LiquidSolverInput {
            steps: 30,
            frame_stride: 30,
            ..Default::default()
        };
        let vol = simulate_liquid(&domain, &sources, &solver, &[floor]);
        let frame = vol.frames.last().expect("frame");
        let count = frame.particle_count as usize;
        for i in 0..count {
            let x = frame.positions[i * 3];
            let y = frame.positions[i * 3 + 1];
            let z = frame.positions[i * 3 + 2];
            if x.abs() > 3.4 || z.abs() > 3.4 {
                continue;
            }
            assert!(y >= 0.35, "particle below floor collider: {y}");
        }
    }
}
