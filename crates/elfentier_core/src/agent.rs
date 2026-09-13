//! Agent-oriented preset and parameter helpers.

use crate::building::BuildingParams;
use crate::collider::ColliderInput;
use crate::graph::{evaluate_liquid_volume, evaluate_smoke_volume, graph_mode, Graph, GraphMode, NodeKind};
use crate::liquid::{export_particle_cache, LiquidDomainInput, LiquidSolverInput, LiquidSourceInput};
use crate::openvdb_io::{export_openvdb_fog, OpenVdbExportResult};
use crate::smoke::{export_density_atlas, SmokeDomainInput, SmokeSolverInput, SmokeSourceInput};
use serde::{Deserialize, Serialize};

/// Metadata for a built-in graph preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// Smoke parameter update request for agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetSmokeParamsRequest {
    pub graph: Graph,
    pub domain: Option<SmokeDomainInput>,
    pub source: Option<SmokeSourceInput>,
    pub solver: Option<SmokeSolverInput>,
    pub collider: Option<ColliderInput>,
}

/// Liquid parameter update request for agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetLiquidParamsRequest {
    pub graph: Graph,
    pub domain: Option<LiquidDomainInput>,
    pub source: Option<LiquidSourceInput>,
    pub solver: Option<LiquidSolverInput>,
    pub collider: Option<ColliderInput>,
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
            id: "smoke_puff".into(),
            name: "Smoke Puff".into(),
            description: "Thin Eulerian smoke (薄い煙) with floor collider".into(),
        },
        PresetInfo {
            id: "smoke_viscous".into(),
            name: "Smoke Viscous".into(),
            description: "Viscous smoke (ねっとり) with slower, thicker motion".into(),
        },
        PresetInfo {
            id: "smoke_sphere".into(),
            name: "Smoke Sphere Obstacle".into(),
            description: "Eulerian smoke rising around a sphere mesh SDF collider".into(),
        },
        PresetInfo {
            id: "ocean_patch".into(),
            name: "Ocean Patch".into(),
            description: "Wide FLIP liquid body (水) with floor collider and gentle waves".into(),
        },
        PresetInfo {
            id: "waterfall".into(),
            name: "Waterfall".into(),
            description: "Elevated liquid source with basin floor collider".into(),
        },
        PresetInfo {
            id: "flood_basin".into(),
            name: "Flood Basin".into(),
            description: "Basin fill (とろみ) with wall collider and higher viscosity".into(),
        },
        PresetInfo {
            id: "liquid_ramp".into(),
            name: "Liquid Ramp".into(),
            description: "FLIP inflow deflected by an inclined ramp mesh SDF collider".into(),
        },
    ]
}

/// Returns a preset graph by id, or None if unknown.
pub fn get_preset(id: &str) -> Option<Graph> {
    match id {
        "shop_street" => Some(Graph::shop_street_preset()),
        "grid_block" => Some(Graph::grid_block_preset()),
        "smoke_puff" => Some(Graph::smoke_puff_preset()),
        "smoke_viscous" => Some(Graph::smoke_viscous_preset()),
        "smoke_sphere" => Some(Graph::smoke_sphere_preset()),
        "ocean_patch" => Some(Graph::ocean_patch_preset()),
        "waterfall" => Some(Graph::waterfall_preset()),
        "flood_basin" => Some(Graph::flood_basin_preset()),
        "liquid_ramp" => Some(Graph::liquid_ramp_preset()),
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

/// Updates smoke domain/source/solver nodes on a smoke graph.
pub fn set_smoke_params(request: &SetSmokeParamsRequest) -> Graph {
    let mut g = request.graph.clone();
    for node in &mut g.nodes {
        if let Some(domain) = request.domain {
            if node.kind == NodeKind::SmokeDomain {
                node.smoke_domain = Some(domain);
            }
        }
        if let Some(source) = request.source {
            if node.kind == NodeKind::SmokeSource {
                node.smoke_source = Some(source);
            }
        }
        if let Some(solver) = request.solver {
            if node.kind == NodeKind::SmokeSolver {
                node.smoke_solver = Some(solver);
            }
        }
        if let Some(collider) = request.collider {
            if node.kind == NodeKind::SmokeCollider {
                node.smoke_collider = Some(collider);
            }
        }
    }
    g
}

/// Updates liquid domain/source/solver nodes on a liquid graph.
pub fn set_liquid_params(request: &SetLiquidParamsRequest) -> Graph {
    let mut g = request.graph.clone();
    for node in &mut g.nodes {
        if let Some(domain) = request.domain {
            if node.kind == NodeKind::LiquidDomain {
                node.liquid_domain = Some(domain);
            }
        }
        if let Some(source) = request.source {
            if node.kind == NodeKind::LiquidSource {
                node.liquid_source = Some(source);
            }
        }
        if let Some(solver) = request.solver {
            if node.kind == NodeKind::LiquidSolver {
                node.liquid_solver = Some(solver);
            }
        }
        if let Some(collider) = request.collider {
            if node.kind == NodeKind::LiquidCollider {
                node.liquid_collider = Some(collider);
            }
        }
    }
    g
}

/// Exports a smoke density atlas for flipbook / engine handoff.
pub fn export_smoke_density(graph: &Graph, path: &str) -> Result<crate::smoke::SmokeExportResult, String> {
    if graph_mode(graph) != GraphMode::Smoke {
        return Err("graph is not a smoke graph (missing SmokeRoot)".into());
    }
    let volume = evaluate_smoke_volume(graph)?;
    export_density_atlas(&volume, path).map_err(|e| e.to_string())
}

/// Exports the latest smoke density frame as an OpenVDB fog FloatGrid (`.vdb`).
pub fn export_smoke_vdb(graph: &Graph, path: &str) -> Result<OpenVdbExportResult, String> {
    if graph_mode(graph) != GraphMode::Smoke {
        return Err("graph is not a smoke graph (missing SmokeRoot)".into());
    }
    let volume = evaluate_smoke_volume(graph)?;
    export_openvdb_fog(&volume, path).map_err(|e| e.to_string())
}

/// Exports a liquid particle cache stub for Unity/game-engine handoff.
pub fn export_liquid_cache(graph: &Graph, path: &str) -> Result<crate::liquid::LiquidExportResult, String> {
    if graph_mode(graph) != GraphMode::Liquid {
        return Err("graph is not a liquid graph (missing LiquidRoot)".into());
    }
    let volume = evaluate_liquid_volume(graph)?;
    export_particle_cache(&volume, path).map_err(|e| e.to_string())
}

/// Writes a cook export bundle directory (manifest + payloads) for engine/DCC import.
pub fn export_cook_bundle_command(
    graph: &Graph,
    path: Option<&str>,
) -> Result<crate::export::ExportBundleResult, String> {
    crate::export::export_cook_bundle(graph, path)
}
