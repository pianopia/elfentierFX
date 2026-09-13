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
}
