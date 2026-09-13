//! Viewport-ready mesh and smoke payloads for native wgpu preview.

use crate::graph::{evaluate_city_instanced, evaluate_smoke, graph_output_kind, Graph, GraphOutputKind};
use crate::placement::InstanceTransform;
use crate::mesh::Mesh;
use crate::smoke::SmokeGrid;
use serde::{Deserialize, Serialize};

/// Column-major 4×4 instance matrix (16 floats).
pub type InstanceMatrix = [f32; 16];

/// Efficient mesh buffers for real-time viewport upload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportMesh {
    /// Flat XYZ positions for the base building mesh.
    pub positions: Vec<f32>,
    /// Triangle indices for the base mesh.
    pub indices: Vec<u32>,
    /// Column-major 4×4 matrices, 16 floats per instance.
    pub instance_matrices: Vec<f32>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub triangle_count: u32,
    pub instance_count: u32,
    pub graph_name: String,
}

/// Smoke volume buffers for native wgpu raymarch / particle impostors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportSmoke {
    pub nx: u32,
    pub ny: u32,
    pub nz: u32,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub density: Vec<f32>,
    pub max_density: f32,
    pub graph_name: String,
}

/// Combined cook payload for the desktop viewport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportCook {
    pub output_kind: GraphOutputKind,
    pub mesh: Option<ViewportMesh>,
    pub smoke: Option<ViewportSmoke>,
}

/// RGBA8 image from native wgpu offscreen smoke preview.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmokePreviewImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Converts an instance transform to a column-major 4×4 matrix matching mesh transform math.
pub fn instance_to_matrix(inst: &InstanceTransform) -> InstanceMatrix {
    let cos = inst.rotation_y.cos();
    let sin = inst.rotation_y.sin();
    let sx = inst.scale.x;
    let sy = inst.scale.y;
    let sz = inst.scale.z;
    [
        cos * sx,
        0.0,
        -sin * sx,
        0.0,
        0.0,
        sy,
        0.0,
        0.0,
        sin * sz,
        0.0,
        cos * sz,
        0.0,
        inst.position.x,
        inst.position.y,
        inst.position.z,
        1.0,
    ]
}

/// Packs a mesh into flat position/index buffers plus per-instance matrices.
pub fn pack_viewport_mesh(
    base: &Mesh,
    instances: &[InstanceTransform],
    graph_name: &str,
) -> ViewportMesh {
    let positions: Vec<f32> = base
        .positions
        .iter()
        .flat_map(|p| [p.x, p.y, p.z])
        .collect();
    let indices = base.indices.clone();
    let instance_matrices: Vec<f32> = instances
        .iter()
        .flat_map(|inst| instance_to_matrix(inst))
        .collect();

    let inst_count = if instances.is_empty() { 1 } else { instances.len() };
    let matrices = if instances.is_empty() {
        instance_to_matrix(&InstanceTransform::default()).to_vec()
    } else {
        instance_matrices
    };

    ViewportMesh {
        positions,
        indices,
        instance_matrices: matrices,
        vertex_count: base.vertex_count(),
        index_count: base.index_count(),
        triangle_count: base.triangle_count(),
        instance_count: inst_count as u32,
        graph_name: graph_name.to_string(),
    }
}

/// Packs a smoke grid for wgpu upload.
pub fn pack_viewport_smoke(grid: &SmokeGrid, graph_name: &str) -> ViewportSmoke {
    ViewportSmoke {
        nx: grid.nx,
        ny: grid.ny,
        nz: grid.nz,
        bounds_min: grid.bounds_min,
        bounds_max: grid.bounds_max,
        density: grid.density.clone(),
        max_density: grid.max_density_value(),
        graph_name: graph_name.to_string(),
    }
}

/// Evaluates the graph and returns viewport buffers (base mesh + instancing).
pub fn cook_viewport_mesh(graph: &Graph) -> Result<ViewportMesh, String> {
    let city = evaluate_city_instanced(graph)?;
    Ok(pack_viewport_mesh(&city.base_mesh, &city.instances, &graph.name))
}

/// Evaluates a smoke graph and returns volume buffers.
pub fn cook_viewport_smoke(graph: &Graph) -> Result<ViewportSmoke, String> {
    let grid = evaluate_smoke(graph)?;
    Ok(pack_viewport_smoke(&grid, &graph.name))
}

/// Evaluates the graph and returns mesh and/or smoke viewport payloads.
pub fn cook_viewport(graph: &Graph) -> Result<ViewportCook, String> {
    match graph_output_kind(graph) {
        GraphOutputKind::City => {
            let mesh = cook_viewport_mesh(graph)?;
            Ok(ViewportCook {
                output_kind: GraphOutputKind::City,
                mesh: Some(mesh),
                smoke: None,
            })
        }
        GraphOutputKind::Smoke => {
            let smoke = cook_viewport_smoke(graph)?;
            Ok(ViewportCook {
                output_kind: GraphOutputKind::Smoke,
                mesh: None,
                smoke: Some(smoke),
            })
        }
    }
}

/// CPU fallback raymarch for tests and headless environments without wgpu.
pub fn raymarch_smoke_cpu(smoke: &ViewportSmoke, width: u32, height: u32) -> SmokePreviewImage {
    let w = width.max(16);
    let h = height.max(16);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let center = [
        (smoke.bounds_min[0] + smoke.bounds_max[0]) * 0.5,
        (smoke.bounds_min[1] + smoke.bounds_max[1]) * 0.5,
        (smoke.bounds_min[2] + smoke.bounds_max[2]) * 0.5,
    ];
    let extent = [
        smoke.bounds_max[0] - smoke.bounds_min[0],
        smoke.bounds_max[1] - smoke.bounds_min[1],
        smoke.bounds_max[2] - smoke.bounds_min[2],
    ]
    .into_iter()
    .fold(1.0_f32, f32::max);

    for py in 0..h {
        for px in 0..w {
            let u = (px as f32 + 0.5) / w as f32;
            let v = (py as f32 + 0.5) / h as f32;
            let ro = [
                center[0] + (u - 0.5) * extent * 1.2,
                center[1] + (v - 0.5) * extent * 0.8,
                center[2] - extent * 1.4,
            ];
            let rd = [0.0, 0.0, 1.0];
            let mut accum = 0.0_f32;
            let steps = 48usize;
            for s in 0..steps {
                let t = (s as f32 + 0.5) / steps as f32;
                let p = [
                    ro[0] + rd[0] * t * extent * 2.2,
                    ro[1] + rd[1] * t * extent * 2.2,
                    ro[2] + rd[2] * t * extent * 2.2,
                ];
                accum += sample_density(smoke, p) * (extent * 2.2 / steps as f32);
            }
            let alpha = (1.0 - (-accum * 2.5).exp()).clamp(0.0, 1.0);
            let i = ((py * w + px) * 4) as usize;
            rgba[i] = (220.0 * alpha) as u8;
            rgba[i + 1] = (210.0 * alpha) as u8;
            rgba[i + 2] = (230.0 * alpha) as u8;
            rgba[i + 3] = 255;
        }
    }
    SmokePreviewImage {
        width: w,
        height: h,
        rgba,
    }
}

fn sample_density(smoke: &ViewportSmoke, p: [f32; 3]) -> f32 {
    let cs = [
        (smoke.bounds_max[0] - smoke.bounds_min[0]) / smoke.nx as f32,
        (smoke.bounds_max[1] - smoke.bounds_min[1]) / smoke.ny as f32,
        (smoke.bounds_max[2] - smoke.bounds_min[2]) / smoke.nz as f32,
    ];
    let g = [
        (p[0] - smoke.bounds_min[0]) / cs[0] - 0.5,
        (p[1] - smoke.bounds_min[1]) / cs[1] - 0.5,
        (p[2] - smoke.bounds_min[2]) / cs[2] - 0.5,
    ];
    if g[0] < 0.0
        || g[1] < 0.0
        || g[2] < 0.0
        || g[0] > smoke.nx as f32 - 1.0
        || g[1] > smoke.ny as f32 - 1.0
        || g[2] > smoke.nz as f32 - 1.0
    {
        return 0.0;
    }
    let x0 = g[0].floor() as u32;
    let y0 = g[1].floor() as u32;
    let z0 = g[2].floor() as u32;
    let idx = |x: u32, y: u32, z: u32| -> usize {
        (x + y * smoke.nx + z * smoke.nx * smoke.ny) as usize
    };
    smoke.density[idx(x0, y0, z0)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    #[test]
    fn viewport_mesh_from_preset() {
        let graph = Graph::shop_street_preset();
        let mesh = cook_viewport_mesh(&graph).expect("viewport cook");
        assert!(mesh.vertex_count > 8);
        assert!(mesh.instance_count >= 4);
        assert_eq!(mesh.instance_matrices.len(), mesh.instance_count as usize * 16);
        assert_eq!(mesh.positions.len(), mesh.vertex_count as usize * 3);
    }

    #[test]
    fn viewport_smoke_from_preset() {
        let graph = Graph::smoke_plume_preset();
        let cook = cook_viewport(&graph).expect("cook");
        assert_eq!(cook.output_kind, GraphOutputKind::Smoke);
        let smoke = cook.smoke.expect("smoke");
        assert!(smoke.density.len() > 0);
        assert!(smoke.max_density > 0.0);
    }

    #[test]
    fn cpu_raymarch_produces_image() {
        let graph = Graph::smoke_plume_preset();
        let smoke = cook_viewport_smoke(&graph).expect("smoke");
        let img = raymarch_smoke_cpu(&smoke, 64, 64);
        assert_eq!(img.rgba.len(), 64 * 64 * 4);
        assert!(img.rgba.iter().any(|&b| b > 0));
    }

    #[test]
    fn identity_matrix_last_element() {
        let m = instance_to_matrix(&InstanceTransform::default());
        assert_eq!(m[15], 1.0);
    }
}
