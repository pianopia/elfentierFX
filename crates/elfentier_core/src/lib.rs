//! elfentierFX core library — procedural graph and mesh primitives.
//!
//! Alpha 0 exposes a minimal API for host integration. OpenVDB and fluid solvers
//! will be added via FFI in later milestones.

pub mod buffer;
pub mod graph;
pub mod mesh;

use serde::{Deserialize, Serialize};

/// Semantic version of the core library.
pub const CORE_VERSION: &str = "0.1.0-alpha.0";

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

/// Creates a unit cube mesh (centered at origin, edge length 1).
///
/// Alpha 0 returns counts only; vertex/index buffers are stubbed for later.
pub fn create_box_mesh() -> MeshStats {
    // Unit cube: 8 unique vertices, 12 triangles (36 indices).
    MeshStats {
        vertex_count: 8,
        index_count: 36,
        triangle_count: 12,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_version_is_non_empty() {
        assert!(!core_version().is_empty());
    }

    #[test]
    fn create_box_mesh_returns_expected_counts() {
        let stats = create_box_mesh();
        assert_eq!(stats.vertex_count, 8);
        assert_eq!(stats.index_count, 36);
        assert_eq!(stats.triangle_count, 12);
    }
}
