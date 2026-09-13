//! Viewport-ready mesh payload with GPU instancing support.

use crate::graph::{evaluate_city_instanced, Graph};
use crate::placement::InstanceTransform;
use crate::mesh::Mesh;
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

/// Evaluates the graph and returns viewport buffers (base mesh + instancing).
pub fn cook_viewport_mesh(graph: &Graph) -> Result<ViewportMesh, String> {
    let city = evaluate_city_instanced(graph)?;
    Ok(pack_viewport_mesh(&city.base_mesh, &city.instances, &graph.name))
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
    fn identity_matrix_last_element() {
        let m = instance_to_matrix(&InstanceTransform::default());
        assert_eq!(m[15], 1.0);
    }
}
