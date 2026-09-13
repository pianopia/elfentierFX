//! Lightweight Eulerian smoke/gas solver (density + velocity on a 3D grid).

use serde::{Deserialize, Serialize};

/// Domain resolution and world-space bounds for the smoke grid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmokeDomainParams {
    pub nx: u32,
    pub ny: u32,
    pub nz: u32,
    /// World-space minimum corner of the domain.
    pub bounds_min: [f32; 3],
    /// World-space maximum corner of the domain.
    pub bounds_max: [f32; 3],
}

impl Default for SmokeDomainParams {
    fn default() -> Self {
        Self {
            nx: 32,
            ny: 48,
            nz: 32,
            bounds_min: [-4.0, 0.0, -4.0],
            bounds_max: [4.0, 8.0, 4.0],
        }
    }
}

/// Emitter placement and rate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmokeSourceParams {
    pub position: [f32; 3],
    pub radius: f32,
    pub emit_rate: f32,
    pub emit_velocity: [f32; 3],
}

impl Default for SmokeSourceParams {
    fn default() -> Self {
        Self {
            position: [0.0, 1.0, 0.0],
            radius: 0.6,
            emit_rate: 2.5,
            emit_velocity: [0.0, 2.0, 0.0],
        }
    }
}

/// Solver timestep settings for one cook pass.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmokeSolverParams {
    pub steps: u32,
    pub dt: f32,
    pub diffusion: f32,
    pub buoyancy: f32,
    pub pressure_iterations: u32,
    pub max_density: f32,
}

impl Default for SmokeSolverParams {
    fn default() -> Self {
        Self {
            steps: 24,
            dt: 0.08,
            diffusion: 0.0002,
            buoyancy: 1.2,
            pressure_iterations: 20,
            max_density: 4.0,
        }
    }
}

/// 3D scalar/vector fields for smoke simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmokeGrid {
    pub nx: u32,
    pub ny: u32,
    pub nz: u32,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub density: Vec<f32>,
    pub vel_x: Vec<f32>,
    pub vel_y: Vec<f32>,
    pub vel_z: Vec<f32>,
}

impl SmokeGrid {
    pub fn new(domain: &SmokeDomainParams) -> Self {
        let nx = domain.nx.max(4);
        let ny = domain.ny.max(4);
        let nz = domain.nz.max(4);
        let count = (nx * ny * nz) as usize;
        Self {
            nx,
            ny,
            nz,
            bounds_min: domain.bounds_min,
            bounds_max: domain.bounds_max,
            density: vec![0.0; count],
            vel_x: vec![0.0; count],
            vel_y: vec![0.0; count],
            vel_z: vec![0.0; count],
        }
    }

    fn idx(&self, x: u32, y: u32, z: u32) -> usize {
        (x + y * self.nx + z * self.nx * self.ny) as usize
    }

    fn cell_size(&self) -> [f32; 3] {
        [
            (self.bounds_max[0] - self.bounds_min[0]) / self.nx as f32,
            (self.bounds_max[1] - self.bounds_min[1]) / self.ny as f32,
            (self.bounds_max[2] - self.bounds_min[2]) / self.nz as f32,
        ]
    }

    fn world_to_grid(&self, p: [f32; 3]) -> [f32; 3] {
        let cs = self.cell_size();
        [
            (p[0] - self.bounds_min[0]) / cs[0] - 0.5,
            (p[1] - self.bounds_min[1]) / cs[1] - 0.5,
            (p[2] - self.bounds_min[2]) / cs[2] - 0.5,
        ]
    }

    fn grid_to_world(&self, g: [f32; 3]) -> [f32; 3] {
        let cs = self.cell_size();
        [
            self.bounds_min[0] + (g[0] + 0.5) * cs[0],
            self.bounds_min[1] + (g[1] + 0.5) * cs[1],
            self.bounds_min[2] + (g[2] + 0.5) * cs[2],
        ]
    }

    fn sample_trilinear(data: &[f32], nx: u32, ny: u32, nz: u32, g: [f32; 3]) -> f32 {
        let gx = g[0].clamp(0.0, nx as f32 - 1.001);
        let gy = g[1].clamp(0.0, ny as f32 - 1.001);
        let gz = g[2].clamp(0.0, nz as f32 - 1.001);
        let x0 = gx.floor() as u32;
        let y0 = gy.floor() as u32;
        let z0 = gz.floor() as u32;
        let x1 = (x0 + 1).min(nx - 1);
        let y1 = (y0 + 1).min(ny - 1);
        let z1 = (z0 + 1).min(nz - 1);
        let tx = gx - x0 as f32;
        let ty = gy - y0 as f32;
        let tz = gz - z0 as f32;

        let idx = |x: u32, y: u32, z: u32| -> usize {
            (x + y * nx + z * nx * ny) as usize
        };

        let c000 = data[idx(x0, y0, z0)];
        let c100 = data[idx(x1, y0, z0)];
        let c010 = data[idx(x0, y1, z0)];
        let c110 = data[idx(x1, y1, z0)];
        let c001 = data[idx(x0, y0, z1)];
        let c101 = data[idx(x1, y0, z1)];
        let c011 = data[idx(x0, y1, z1)];
        let c111 = data[idx(x1, y1, z1)];

        let c00 = c000 * (1.0 - tx) + c100 * tx;
        let c10 = c010 * (1.0 - tx) + c110 * tx;
        let c01 = c001 * (1.0 - tx) + c101 * tx;
        let c11 = c011 * (1.0 - tx) + c111 * tx;
        let c0 = c00 * (1.0 - ty) + c10 * ty;
        let c1 = c01 * (1.0 - ty) + c11 * ty;
        c0 * (1.0 - tz) + c1 * tz
    }

    fn sample_velocity(&self, g: [f32; 3]) -> [f32; 3] {
        [
            Self::sample_trilinear(&self.vel_x, self.nx, self.ny, self.nz, g),
            Self::sample_trilinear(&self.vel_y, self.nx, self.ny, self.nz, g),
            Self::sample_trilinear(&self.vel_z, self.nx, self.ny, self.nz, g),
        ]
    }

    fn advect_scalar(&self, field: &[f32], dt: f32) -> Vec<f32> {
        let mut out = vec![0.0; field.len()];
        for z in 0..self.nz {
            for y in 0..self.ny {
                for x in 0..self.nx {
                    let g = [x as f32, y as f32, z as f32];
                    let vel = self.sample_velocity(g);
                    let cs = self.cell_size();
                    let vel_grid = [
                        vel[0] / cs[0],
                        vel[1] / cs[1],
                        vel[2] / cs[2],
                    ];
                    let prev = [
                        g[0] - vel_grid[0] * dt,
                        g[1] - vel_grid[1] * dt,
                        g[2] - vel_grid[2] * dt,
                    ];
                    out[self.idx(x, y, z)] =
                        Self::sample_trilinear(field, self.nx, self.ny, self.nz, prev);
                }
            }
        }
        out
    }

    fn advect_velocity(&self, dt: f32) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let mut vx = vec![0.0; self.vel_x.len()];
        let mut vy = vec![0.0; self.vel_y.len()];
        let mut vz = vec![0.0; self.vel_z.len()];
        for z in 0..self.nz {
            for y in 0..self.ny {
                for x in 0..self.nx {
                    let g = [x as f32, y as f32, z as f32];
                    let vel = self.sample_velocity(g);
                    let cs = self.cell_size();
                    let vel_grid = [
                        vel[0] / cs[0],
                        vel[1] / cs[1],
                        vel[2] / cs[2],
                    ];
                    let prev = [
                        g[0] - vel_grid[0] * dt,
                        g[1] - vel_grid[1] * dt,
                        g[2] - vel_grid[2] * dt,
                    ];
                    let i = self.idx(x, y, z);
                    vx[i] = Self::sample_trilinear(&self.vel_x, self.nx, self.ny, self.nz, prev);
                    vy[i] = Self::sample_trilinear(&self.vel_y, self.nx, self.ny, self.nz, prev);
                    vz[i] = Self::sample_trilinear(&self.vel_z, self.nx, self.ny, self.nz, prev);
                }
            }
        }
        (vx, vy, vz)
    }

    fn diffuse_scalar(&self, field: &[f32], diffusion: f32, dt: f32) -> Vec<f32> {
        if diffusion <= 0.0 {
            return field.to_vec();
        }
        let cs = self.cell_size();
        let a = [
            dt * diffusion / (cs[0] * cs[0]),
            dt * diffusion / (cs[1] * cs[1]),
            dt * diffusion / (cs[2] * cs[2]),
        ];
        let mut out = field.to_vec();
        for _ in 0..2 {
            let mut next = out.clone();
            for z in 1..self.nz - 1 {
                for y in 1..self.ny - 1 {
                    for x in 1..self.nx - 1 {
                        let i = self.idx(x, y, z);
                        let lap = out[self.idx(x - 1, y, z)] + out[self.idx(x + 1, y, z)]
                            + out[self.idx(x, y - 1, z)]
                            + out[self.idx(x, y + 1, z)]
                            + out[self.idx(x, y, z - 1)]
                            + out[self.idx(x, y, z + 1)]
                            - 6.0 * out[i];
                        next[i] = out[i] + a[0] * lap;
                    }
                }
            }
            out = next;
        }
        out
    }

    fn apply_buoyancy(&mut self, buoyancy: f32, dt: f32) {
        for z in 0..self.nz {
            for y in 0..self.ny {
                for x in 0..self.nx {
                    let i = self.idx(x, y, z);
                    self.vel_y[i] += buoyancy * self.density[i] * dt;
                }
            }
        }
    }

    fn divergence(&self) -> Vec<f32> {
        let cs = self.cell_size();
        let mut div = vec![0.0; self.density.len()];
        for z in 1..self.nz - 1 {
            for y in 1..self.ny - 1 {
                for x in 1..self.nx - 1 {
                    let i = self.idx(x, y, z);
                    div[i] = (self.vel_x[self.idx(x + 1, y, z)] - self.vel_x[self.idx(x - 1, y, z)])
                        / (2.0 * cs[0])
                        + (self.vel_y[self.idx(x, y + 1, z)] - self.vel_y[self.idx(x, y - 1, z)])
                            / (2.0 * cs[1])
                        + (self.vel_z[self.idx(x, y, z + 1)] - self.vel_z[self.idx(x, y, z - 1)])
                            / (2.0 * cs[2]);
                }
            }
        }
        div
    }

    fn project(&mut self, iterations: u32) {
        let div = self.divergence();
        let cs = self.cell_size();
        let mut pressure = vec![0.0; self.density.len()];
        let denom = 2.0 * (1.0 / (cs[0] * cs[0]) + 1.0 / (cs[1] * cs[1]) + 1.0 / (cs[2] * cs[2]));

        for _ in 0..iterations {
            let mut next = pressure.clone();
            for z in 1..self.nz - 1 {
                for y in 1..self.ny - 1 {
                    for x in 1..self.nx - 1 {
                        let i = self.idx(x, y, z);
                        let lap = pressure[self.idx(x - 1, y, z)] + pressure[self.idx(x + 1, y, z)]
                            + pressure[self.idx(x, y - 1, z)]
                            + pressure[self.idx(x, y + 1, z)]
                            + pressure[self.idx(x, y, z - 1)]
                            + pressure[self.idx(x, y, z + 1)]
                            - 6.0 * pressure[i];
                        next[i] = (lap - div[i] * cs[0] * cs[0]) / denom;
                    }
                }
            }
            pressure = next;
        }

        for z in 1..self.nz - 1 {
            for y in 1..self.ny - 1 {
                for x in 1..self.nx - 1 {
                    let i = self.idx(x, y, z);
                    self.vel_x[i] -= (pressure[self.idx(x + 1, y, z)] - pressure[self.idx(x - 1, y, z)])
                        / (2.0 * cs[0]);
                    self.vel_y[i] -= (pressure[self.idx(x, y + 1, z)] - pressure[self.idx(x, y - 1, z)])
                        / (2.0 * cs[1]);
                    self.vel_z[i] -= (pressure[self.idx(x, y, z + 1)] - pressure[self.idx(x, y, z - 1)])
                        / (2.0 * cs[2]);
                }
            }
        }
    }

    fn emit(&mut self, source: &SmokeSourceParams, dt: f32) {
        let gpos = self.world_to_grid(source.position);
        let cs = self.cell_size();
        let radius_cells = source.radius / cs[0].max(cs[1]).max(cs[2]);
        let r2 = radius_cells * radius_cells;

        for z in 0..self.nz {
            for y in 0..self.ny {
                for x in 0..self.nx {
                    let dx = x as f32 - gpos[0];
                    let dy = y as f32 - gpos[1];
                    let dz = z as f32 - gpos[2];
                    let d2 = dx * dx + dy * dy + dz * dz;
                    if d2 > r2 {
                        continue;
                    }
                    let falloff = (1.0 - (d2 / r2).sqrt()).max(0.0);
                    let i = self.idx(x, y, z);
                    self.density[i] += source.emit_rate * falloff * dt;
                    self.vel_x[i] += source.emit_velocity[0] * falloff * dt;
                    self.vel_y[i] += source.emit_velocity[1] * falloff * dt;
                    self.vel_z[i] += source.emit_velocity[2] * falloff * dt;
                }
            }
        }
    }

    fn clamp_density(&mut self, max_density: f32) {
        for d in &mut self.density {
            *d = d.clamp(0.0, max_density);
        }
    }

    fn enforce_boundaries(&mut self) {
        for z in 0..self.nz {
            for y in 0..self.ny {
                for x in 0..self.nx {
                    if x == 0
                        || y == 0
                        || z == 0
                        || x + 1 == self.nx
                        || y + 1 == self.ny
                        || z + 1 == self.nz
                    {
                        let i = self.idx(x, y, z);
                        self.density[i] = 0.0;
                        self.vel_x[i] = 0.0;
                        self.vel_y[i] = 0.0;
                        self.vel_z[i] = 0.0;
                    }
                }
            }
        }
    }

    /// Returns true if all field values are finite.
    pub fn is_finite(&self) -> bool {
        self.density.iter().all(|v| v.is_finite())
            && self.vel_x.iter().all(|v| v.is_finite())
            && self.vel_y.iter().all(|v| v.is_finite())
            && self.vel_z.iter().all(|v| v.is_finite())
    }

    /// Maximum density in the grid.
    pub fn max_density_value(&self) -> f32 {
        self.density.iter().copied().fold(0.0_f32, f32::max)
    }
}

/// Runs the smoke solver for the given domain, source, and solver settings.
pub fn simulate_smoke(
    domain: &SmokeDomainParams,
    source: &SmokeSourceParams,
    solver: &SmokeSolverParams,
) -> SmokeGrid {
    let mut grid = SmokeGrid::new(domain);
    for _ in 0..solver.steps.max(1) {
        grid.emit(source, solver.dt);
        grid.density = grid.advect_scalar(&grid.density, solver.dt);
        let (vx, vy, vz) = grid.advect_velocity(solver.dt);
        grid.vel_x = vx;
        grid.vel_y = vy;
        grid.vel_z = vz;
        grid.vel_x = grid.diffuse_scalar(&grid.vel_x, solver.diffusion, solver.dt);
        grid.vel_y = grid.diffuse_scalar(&grid.vel_y, solver.diffusion, solver.dt);
        grid.vel_z = grid.diffuse_scalar(&grid.vel_z, solver.diffusion, solver.dt);
        grid.apply_buoyancy(solver.buoyancy, solver.dt);
        grid.project(solver.pressure_iterations);
        grid.density = grid.diffuse_scalar(&grid.density, solver.diffusion, solver.dt);
        grid.clamp_density(solver.max_density);
        grid.enforce_boundaries();
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_produces_finite_values() {
        let grid = simulate_smoke(
            &SmokeDomainParams::default(),
            &SmokeSourceParams::default(),
            &SmokeSolverParams {
                steps: 8,
                ..Default::default()
            },
        );
        assert!(grid.is_finite(), "simulation produced NaN or Inf");
    }

    #[test]
    fn density_stays_bounded() {
        let solver = SmokeSolverParams {
            steps: 16,
            max_density: 2.0,
            ..Default::default()
        };
        let grid = simulate_smoke(
            &SmokeDomainParams::default(),
            &SmokeSourceParams {
                emit_rate: 8.0,
                ..Default::default()
            },
            &solver,
        );
        assert!(grid.is_finite());
        assert!(
            grid.density.iter().all(|&d| d >= 0.0 && d <= solver.max_density + 1e-5),
            "density exceeded max"
        );
    }

    #[test]
    fn smoke_accumulates_at_source() {
        let grid = simulate_smoke(
            &SmokeDomainParams::default(),
            &SmokeSourceParams::default(),
            &SmokeSolverParams {
                steps: 4,
                ..Default::default()
            },
        );
        assert!(grid.max_density_value() > 0.01);
    }
}
