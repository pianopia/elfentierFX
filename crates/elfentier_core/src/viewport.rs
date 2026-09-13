//! Viewport-ready mesh payload with GPU instancing and smoke particle support.

use crate::graph::{evaluate_city_instanced, evaluate_liquid_volume, evaluate_smoke_volume, Graph, GraphMode};
use crate::liquid::LiquidVolume;
use crate::placement::InstanceTransform;
use crate::mesh::Mesh;
use crate::smoke::SmokeVolume;
use serde::{Deserialize, Serialize};

/// Column-major 4×4 instance matrix (16 floats).
pub type InstanceMatrix = [f32; 16];

/// One animation frame of soft smoke impostor particles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportSmokeFrame {
    pub positions: Vec<f32>,
    pub sizes: Vec<f32>,
    pub opacities: Vec<f32>,
    pub particle_count: u32,
}

/// One animation frame of liquid particles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportLiquidFrame {
    pub positions: Vec<f32>,
    pub radii: Vec<f32>,
    pub opacities: Vec<f32>,
    pub particle_count: u32,
}

/// Animated liquid preview for the 3D viewport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportLiquid {
    pub frames: Vec<ViewportLiquidFrame>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub frame_count: u32,
    pub fps: f32,
    pub stats: crate::liquid::LiquidStats,
}

/// Animated smoke preview for the 3D viewport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportSmoke {
    pub frames: Vec<ViewportSmokeFrame>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub frame_count: u32,
    pub fps: f32,
    pub stats: crate::smoke::SmokeStats,
}

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
    /// Present when the graph cooks a smoke volume instead of (or alongside) city geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoke: Option<ViewportSmoke>,
    /// Present when the graph cooks a liquid volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid: Option<ViewportLiquid>,
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
    smoke: Option<ViewportSmoke>,
    liquid: Option<ViewportLiquid>,
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
        smoke,
        liquid,
    }
}

pub fn pack_viewport_smoke(volume: &SmokeVolume, graph_name: &str) -> ViewportMesh {
    let frames: Vec<ViewportSmokeFrame> = volume
        .frames
        .iter()
        .map(|f| ViewportSmokeFrame {
            positions: f.positions.clone(),
            sizes: f.sizes.clone(),
            opacities: f.opacities.clone(),
            particle_count: f.particle_count,
        })
        .collect();

    let smoke = ViewportSmoke {
        frame_count: frames.len() as u32,
        fps: 12.0,
        bounds_min: [
            volume.bounds_min.x,
            volume.bounds_min.y,
            volume.bounds_min.z,
        ],
        bounds_max: [
            volume.bounds_max.x,
            volume.bounds_max.y,
            volume.bounds_max.z,
        ],
        stats: volume.stats,
        frames,
    };

    pack_viewport_mesh(&Mesh::unnamed(), &[], graph_name, Some(smoke), None)
}

pub fn pack_viewport_liquid(volume: &LiquidVolume, graph_name: &str) -> ViewportMesh {
    let frames: Vec<ViewportLiquidFrame> = volume
        .frames
        .iter()
        .map(|f| ViewportLiquidFrame {
            positions: f.positions.clone(),
            radii: f.radii.clone(),
            opacities: f.opacities.clone(),
            particle_count: f.particle_count,
        })
        .collect();

    let liquid = ViewportLiquid {
        frame_count: frames.len() as u32,
        fps: 12.0,
        bounds_min: [
            volume.bounds_min.x,
            volume.bounds_min.y,
            volume.bounds_min.z,
        ],
        bounds_max: [
            volume.bounds_max.x,
            volume.bounds_max.y,
            volume.bounds_max.z,
        ],
        stats: volume.stats,
        frames,
    };

    pack_viewport_mesh(&Mesh::unnamed(), &[], graph_name, None, Some(liquid))
}

/// Evaluates the graph and returns viewport buffers (city mesh and/or smoke).
pub fn cook_viewport_mesh(graph: &Graph) -> Result<ViewportMesh, String> {
    match graph_mode(graph) {
        GraphMode::Liquid => {
            let volume = evaluate_liquid_volume(graph)?;
            Ok(pack_viewport_liquid(&volume, &graph.name))
        }
        GraphMode::Smoke => {
            let volume = evaluate_smoke_volume(graph)?;
            Ok(pack_viewport_smoke(&volume, &graph.name))
        }
        GraphMode::City => {
            let city = evaluate_city_instanced(graph)?;
            Ok(pack_viewport_mesh(&city.base_mesh, &city.instances, &graph.name, None, None))
        }
    }
}

fn graph_mode(graph: &Graph) -> GraphMode {
    crate::graph::graph_mode(graph)
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
        assert!(mesh.smoke.is_none());
    }

    #[test]
    fn smoke_viewport_from_preset() {
        let graph = Graph::smoke_puff_preset();
        let mesh = cook_viewport_mesh(&graph).expect("smoke viewport");
        let smoke = mesh.smoke.expect("smoke payload");
        assert!(smoke.frame_count >= 2);
        assert!(smoke.frames[0].particle_count > 0);
    }

    #[test]
    fn liquid_viewport_from_ocean_preset() {
        let graph = Graph::ocean_patch_preset();
        let mesh = cook_viewport_mesh(&graph).expect("liquid viewport");
        let liquid = mesh.liquid.expect("liquid payload");
        assert!(liquid.frame_count >= 2);
        assert!(liquid.frames[0].particle_count > 0);
    }

    #[test]
    fn identity_matrix_last_element() {
        let m = instance_to_matrix(&InstanceTransform::default());
        assert_eq!(m[15], 1.0);
    }
}
