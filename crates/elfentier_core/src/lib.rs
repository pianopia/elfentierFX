//! elfentierFX core library — procedural graph and mesh primitives.
//!
//! Alpha 1 adds building generation, placement, graph cook, and glTF export.

pub mod buffer;
pub mod building;
pub mod export;
pub mod graph;
pub mod mesh;
pub mod placement;

use mesh::create_unit_box_mesh;
use serde::{Deserialize, Serialize};

/// Semantic version of the core library.
pub const CORE_VERSION: &str = "0.1.0-alpha.1";

/// Returns the core library version string.
pub fn core_version() -> &'static str {
    CORE_VERSION
}

/// Summary statistics for a generated mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshStats {
    pub vertex_count: u32,
    pub index_count: u32,
    pub triangle_count: u32,
}

impl From<&mesh::Mesh> for MeshStats {
    fn from(mesh: &mesh::Mesh) -> Self {
        Self {
            vertex_count: mesh.vertex_count(),
            index_count: mesh.index_count(),
            triangle_count: mesh.triangle_count(),
        }
    }
}

/// Creates a unit cube mesh (centered at origin, edge length 1).
pub fn create_box_mesh() -> MeshStats {
    MeshStats::from(&create_unit_box_mesh())
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph::Graph;

    #[test]
    fn core_version_is_alpha_one() {
        assert!(core_version().contains("alpha.1"));
    }

    #[test]
    fn create_box_mesh_returns_expected_counts() {
        let stats = create_box_mesh();
        assert_eq!(stats.vertex_count, 8);
        assert_eq!(stats.index_count, 36);
        assert_eq!(stats.triangle_count, 12);
    }

    #[test]
    fn shop_street_preset_cooks() {
        let graph = Graph::shop_street_preset();
        let result = graph::cook_graph(&graph).expect("cook");
        assert!(result.instance_count > 0);
        assert!(result.vertex_count > 8);
    }
}
