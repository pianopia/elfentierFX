//! Template-based natural-language summary of the current graph.

use crate::graph::{Graph, NodeKind};

/// Generates a short human-readable summary of the graph structure and parameters.
pub fn explain_graph(graph: &Graph) -> String {
    let mut lines = vec![format!("Graph: {}", graph.name)];

    if let Some(params) = graph
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::BuildingParams)
        .and_then(|n| n.building_params)
    {
        lines.push(format!(
            "Building — {} floors ({:.1}m tall), {:.1}×{:.1}m footprint, {:.0}% window density, seed {}",
            params.floors,
            params.total_height(),
            params.width,
            params.depth,
            params.window_density * 100.0,
            params.seed,
        ));
    }

    let placement = graph.nodes.iter().find(|n| {
        n.kind == NodeKind::PlaceAlongPath || n.kind == NodeKind::FillGrid
    });
    match placement.map(|n| n.kind) {
        Some(NodeKind::PlaceAlongPath) => {
            if let Some(node) = placement {
                let path = node.path_input.unwrap_or_default();
                lines.push(format!(
                    "Placement — along path from ({:.0},{:.0},{:.0}) to ({:.0},{:.0},{:.0}), spacing {:.1}m",
                    path.start.x,
                    path.start.y,
                    path.start.z,
                    path.end.x,
                    path.end.y,
                    path.end.z,
                    path.spacing,
                ));
            }
        }
        Some(NodeKind::FillGrid) => {
            if let Some(node) = placement {
                let grid = node.grid_input.unwrap_or_default();
                lines.push(format!(
                    "Placement — {}×{} grid, lot {:.1}×{:.1}m, spacing {:.1}m",
                    grid.cols,
                    grid.rows,
                    grid.lot_width,
                    grid.lot_depth,
                    grid.spacing,
                ));
            }
        }
        _ => lines.push("Placement — direct mesh (no instancing node)".into()),
    }

    if graph.nodes.iter().any(|n| n.kind == NodeKind::LiquidDomain) {
        if let Some(node) = graph.nodes.iter().find(|n| n.kind == NodeKind::LiquidDomain) {
            let domain = node.liquid_domain.unwrap_or_default();
            lines.push(format!(
                "Liquid domain — {}³ grid, {} initial particles, radius {:.2}m, seed {}",
                domain.resolution,
                domain.initial_particles,
                domain.particle_radius,
                domain.seed,
            ));
        }
        if let Some(node) = graph.nodes.iter().find(|n| n.kind == NodeKind::LiquidSolver) {
            let solver = node.liquid_solver.unwrap_or_default();
            lines.push(format!(
                "Liquid solver — {} steps, gravity {:.1}, FLIP {:.0}%, viscosity {:.2}, wave {:.2}",
                solver.steps,
                solver.gravity,
                solver.flip_ratio * 100.0,
                solver.viscosity,
                solver.wave_amplitude,
            ));
        }
        if let Some(node) = graph.nodes.iter().find(|n| n.kind == NodeKind::LiquidCollider) {
            let collider = node.liquid_collider.unwrap_or_default();
            lines.push(format!(
                "Liquid collider — AABB ({:.1},{:.1},{:.1})→({:.1},{:.1},{:.1})",
                collider.bounds_min.x,
                collider.bounds_min.y,
                collider.bounds_min.z,
                collider.bounds_max.x,
                collider.bounds_max.y,
                collider.bounds_max.z,
            ));
        }
    }

    if graph.nodes.iter().any(|n| n.kind == NodeKind::SmokeDomain) {
        if let Some(node) = graph.nodes.iter().find(|n| n.kind == NodeKind::SmokeDomain) {
            let domain = node.smoke_domain.unwrap_or_default();
            lines.push(format!(
                "Smoke domain — {}³ grid, bounds ({:.0},{:.0},{:.0})→({:.0},{:.0},{:.0}), seed {}",
                domain.resolution,
                domain.bounds_min.x,
                domain.bounds_min.y,
                domain.bounds_min.z,
                domain.bounds_max.x,
                domain.bounds_max.y,
                domain.bounds_max.z,
                domain.seed,
            ));
        }
        if let Some(node) = graph.nodes.iter().find(|n| n.kind == NodeKind::SmokeSolver) {
            let solver = node.smoke_solver.unwrap_or_default();
            lines.push(format!(
                "Smoke solver — {} steps, buoyancy {:.1}, dissipation {:.3}, viscosity {:.2}",
                solver.steps, solver.buoyancy, solver.dissipation, solver.viscosity,
            ));
        }
        if let Some(node) = graph.nodes.iter().find(|n| n.kind == NodeKind::SmokeCollider) {
            let collider = node.smoke_collider.unwrap_or_default();
            lines.push(format!(
                "Smoke collider — AABB ({:.1},{:.1},{:.1})→({:.1},{:.1},{:.1})",
                collider.bounds_min.x,
                collider.bounds_min.y,
                collider.bounds_min.z,
                collider.bounds_max.x,
                collider.bounds_max.y,
                collider.bounds_max.z,
            ));
        }
    }

    lines.push(format!(
        "Nodes — {} nodes, {} edges",
        graph.nodes.len(),
        graph.edges.len(),
    ));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    #[test]
    fn explains_shop_preset() {
        let text = explain_graph(&Graph::shop_street_preset());
        assert!(text.contains("Shop Street"));
        assert!(text.contains("floors"));
    }

    #[test]
    fn explains_smoke_preset() {
        let text = explain_graph(&Graph::smoke_puff_preset());
        assert!(text.contains("Smoke Puff"));
        assert!(text.contains("Smoke domain"));
    }

    #[test]
    fn explains_ocean_preset() {
        let text = explain_graph(&Graph::ocean_patch_preset());
        assert!(text.contains("Ocean Patch"));
        assert!(text.contains("Liquid domain"));
    }
}
