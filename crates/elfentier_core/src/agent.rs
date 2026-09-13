//! Agent-oriented preset and parameter helpers.

use crate::building::BuildingParams;
use crate::graph::{Graph, NodeKind};
use serde::{Deserialize, Serialize};

/// Metadata for a built-in graph preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// Lists available graph presets for agents and UI.
pub fn list_presets() -> Vec<PresetInfo> {
    vec![
        PresetInfo {
            id: "shop_street".into(),
            name: "Shop Street".into(),
            description: "Building mesh placed along a straight street path".into(),
        },
        PresetInfo {
            id: "grid_block".into(),
            name: "Grid Block".into(),
            description: "Building mesh filling a rectangular grid of lots".into(),
        },
        PresetInfo {
            id: "smoke_plume".into(),
            name: "Smoke Plume".into(),
            description: "Eulerian smoke domain, source, and solver chain".into(),
        },
    ]
}

/// Returns a preset graph by id, or None if unknown.
pub fn get_preset(id: &str) -> Option<Graph> {
    match id {
        "shop_street" => Some(Graph::shop_street_preset()),
        "grid_block" => Some(Graph::grid_block_preset()),
        "smoke_plume" => Some(Graph::smoke_plume_preset()),
        _ => None,
    }
}

/// Updates building params on a specific node (or the first BuildingParams node).
pub fn set_params(graph: &Graph, node_id: Option<&str>, params: BuildingParams) -> Graph {
    let mut g = graph.clone();
    for node in &mut g.nodes {
        if node.kind != NodeKind::BuildingParams {
            continue;
        }
        if node_id.is_some() && node.id.0 != node_id.unwrap() {
            continue;
        }
        node.building_params = Some(params);
        break;
    }
    g
}
