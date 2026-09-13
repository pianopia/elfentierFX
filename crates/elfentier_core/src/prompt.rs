//! Local natural-language prompt interpreter (no LLM required).

use crate::building::BuildingParams;
use crate::graph::{Graph, NodeId, NodeKind};
use crate::placement::GridInput;
use crate::liquid::{LiquidSolverInput, LiquidSourceInput};
use crate::smoke::{SmokeSolverInput, SmokeSourceInput};
use serde::{Deserialize, Serialize};

/// High-level intent recognized from a user prompt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptIntent {
    LoadPreset { preset_id: String },
    SetFloors { floors: u32 },
    SetSeed { seed: u64 },
    AdjustWindows { delta: f32 },
    SwitchPlacement { mode: PlacementMode },
    SetGraphName { name: String },
    IncreaseSmoke { emission_delta: f32, step_delta: u32 },
    IncreaseLiquid { emission_delta: f32, wave_delta: f32, step_delta: u32 },
    Unknown { raw: String },
}

/// Placement mode for graph edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementMode {
    AlongPath,
    Grid,
}

/// A concrete mutation to apply to the graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdit {
    LoadPreset { preset_id: String },
    SetBuildingParams { node_id: String, params: BuildingParams },
    SwitchPlacement { mode: PlacementMode },
    SetGraphName { name: String },
    SetSmokeParams {
        source_node_id: String,
        source: SmokeSourceInput,
        solver_node_id: String,
        solver: SmokeSolverInput,
    },
    SetLiquidParams {
        source_node_id: String,
        source: LiquidSourceInput,
        solver_node_id: String,
        solver: LiquidSolverInput,
    },
}

/// Result of interpreting and applying a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPromptResult {
    pub graph: Graph,
    pub intents: Vec<PromptIntent>,
    pub edits: Vec<GraphEdit>,
    pub summary: String,
}

/// Interprets natural-language input (JP + EN) into graph edit intents.
pub fn interpret_prompt(text: &str) -> Vec<PromptIntent> {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() {
        return vec![];
    }

    let mut intents = Vec::new();

    // Smoke puff / 煙
    if contains_any(
        &normalized,
        &[
            "煙",
            "smoke",
            "smoke puff",
            "smoke preset",
            "ガス",
            "gas puff",
        ],
    ) {
        intents.push(PromptIntent::LoadPreset {
            preset_id: "smoke_puff".into(),
        });
    }

    if contains_any(
        &normalized,
        &["もっと煙", "more smoke", "denser smoke", "煙を増", "煙もっと"],
    ) {
        intents.push(PromptIntent::IncreaseSmoke {
            emission_delta: 1.2,
            step_delta: 12,
        });
    }

    // Ocean / 海
    if contains_any(
        &normalized,
        &["海", "ocean", "ocean patch", "ocean_patch", "大洋", "海面"],
    ) {
        intents.push(PromptIntent::LoadPreset {
            preset_id: "ocean_patch".into(),
        });
    }

    // Waterfall / 滝
    if contains_any(
        &normalized,
        &["滝", "waterfall", "cascade", "瀑布"],
    ) {
        intents.push(PromptIntent::LoadPreset {
            preset_id: "waterfall".into(),
        });
    }

    // Flood / 洪水
    if contains_any(
        &normalized,
        &["洪水", "flood", "flood basin", "flood_basin", "浸水", "水害"],
    ) {
        intents.push(PromptIntent::LoadPreset {
            preset_id: "flood_basin".into(),
        });
    }

    // Liquid intensity / 水位上げ / もっと激しく
    if contains_any(
        &normalized,
        &[
            "水位上げ",
            "raise water",
            "more water",
            "水を増",
            "もっと水",
            "more liquid",
        ],
    ) {
        intents.push(PromptIntent::IncreaseLiquid {
            emission_delta: 4.0,
            wave_delta: 0.0,
            step_delta: 12,
        });
    } else if contains_any(
        &normalized,
        &[
            "もっと激しく",
            "more intense",
            "splashier",
            "激しく",
            "波を大きく",
            "bigger waves",
        ],
    ) {
        intents.push(PromptIntent::IncreaseLiquid {
            emission_delta: 2.0,
            wave_delta: 0.15,
            step_delta: 8,
        });
    }

    // Shop street / 商店街
    if contains_any(&normalized, &["商店街", "shop street", "shop-street", "shotengai"]) {
        intents.push(PromptIntent::LoadPreset {
            preset_id: "shop_street".into(),
        });
    }

    // Grid placement / グリッド
    if contains_any(
        &normalized,
        &["グリッド", "grid", "grid placement", "グリッド配置"],
    ) {
        intents.push(PromptIntent::SwitchPlacement {
            mode: PlacementMode::Grid,
        });
    }

    // Path / street placement
    if contains_any(
        &normalized,
        &["ストリート", "street", "path placement", "沿道", "パス配置"],
    ) && !contains_any(&normalized, &["grid", "グリッド"]) {
        intents.push(PromptIntent::SwitchPlacement {
            mode: PlacementMode::AlongPath,
        });
    }

    // Floors: 「階数を5に」, "5 floors", "floors 5"
    if let Some(floors) = parse_floors(&normalized) {
        intents.push(PromptIntent::SetFloors { floors });
    }

    // Seed change
    if contains_any(
        &normalized,
        &["seed変え", "seed change", "change seed", "random seed", "シード", "seedを変"],
    ) {
        let seed = pseudo_random_seed(&normalized);
        intents.push(PromptIntent::SetSeed { seed });
    }

    // Window density
    if contains_any(&normalized, &["もっと窓", "more window", "more windows", "窓を増"]) {
        intents.push(PromptIntent::AdjustWindows { delta: 0.12 });
    } else if contains_any(&normalized, &["窓を減", "less window", "fewer window"]) {
        intents.push(PromptIntent::AdjustWindows { delta: -0.12 });
    }

    if intents.is_empty() {
        intents.push(PromptIntent::Unknown {
            raw: text.trim().to_string(),
        });
    }

    intents
}

/// Converts intents into concrete graph edits.
pub fn intents_to_edits(graph: &Graph, intents: &[PromptIntent]) -> Vec<GraphEdit> {
    let params_node = graph
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::BuildingParams);
    let smoke_source_node = graph.nodes.iter().find(|n| n.kind == NodeKind::SmokeSource);
    let smoke_solver_node = graph.nodes.iter().find(|n| n.kind == NodeKind::SmokeSolver);
    let liquid_source_node = graph.nodes.iter().find(|n| n.kind == NodeKind::LiquidSource);
    let liquid_solver_node = graph.nodes.iter().find(|n| n.kind == NodeKind::LiquidSolver);
    let mut edits = Vec::new();

    for intent in intents {
        match intent {
            PromptIntent::LoadPreset { preset_id } => {
                edits.push(GraphEdit::LoadPreset {
                    preset_id: preset_id.clone(),
                });
            }
            PromptIntent::SetFloors { floors } => {
                if let Some(node) = params_node {
                    let params = node.building_params.unwrap_or_default();
                    edits.push(GraphEdit::SetBuildingParams {
                        node_id: node.id.0.clone(),
                        params: BuildingParams {
                            floors: *floors,
                            ..params
                        },
                    });
                }
            }
            PromptIntent::SetSeed { seed } => {
                if let Some(node) = params_node {
                    let params = node.building_params.unwrap_or_default();
                    edits.push(GraphEdit::SetBuildingParams {
                        node_id: node.id.0.clone(),
                        params: BuildingParams { seed: *seed, ..params },
                    });
                }
            }
            PromptIntent::AdjustWindows { delta } => {
                if let Some(node) = params_node {
                    let params = node.building_params.unwrap_or_default();
                    let density = (params.window_density + delta).clamp(0.05, 0.95);
                    edits.push(GraphEdit::SetBuildingParams {
                        node_id: node.id.0.clone(),
                        params: BuildingParams {
                            window_density: density,
                            ..params
                        },
                    });
                }
            }
            PromptIntent::SwitchPlacement { mode } => {
                edits.push(GraphEdit::SwitchPlacement { mode: *mode });
            }
            PromptIntent::SetGraphName { name } => {
                edits.push(GraphEdit::SetGraphName { name: name.clone() });
            }
            PromptIntent::IncreaseSmoke {
                emission_delta,
                step_delta,
            } => {
                if let (Some(src), Some(slv)) = (smoke_source_node, smoke_solver_node) {
                    let source = src.smoke_source.unwrap_or_default();
                    let solver = slv.smoke_solver.unwrap_or_default();
                    edits.push(GraphEdit::SetSmokeParams {
                        source_node_id: src.id.0.clone(),
                        source: SmokeSourceInput {
                            emission_rate: source.emission_rate + emission_delta,
                            ..source
                        },
                        solver_node_id: slv.id.0.clone(),
                        solver: SmokeSolverInput {
                            steps: solver.steps + step_delta,
                            ..solver
                        },
                    });
                }
            }
            PromptIntent::IncreaseLiquid {
                emission_delta,
                wave_delta,
                step_delta,
            } => {
                if let (Some(src), Some(slv)) = (liquid_source_node, liquid_solver_node) {
                    let source = src.liquid_source.unwrap_or_default();
                    let solver = slv.liquid_solver.unwrap_or_default();
                    edits.push(GraphEdit::SetLiquidParams {
                        source_node_id: src.id.0.clone(),
                        source: LiquidSourceInput {
                            emission_rate: source.emission_rate + emission_delta,
                            ..source
                        },
                        solver_node_id: slv.id.0.clone(),
                        solver: LiquidSolverInput {
                            steps: solver.steps + step_delta,
                            wave_amplitude: solver.wave_amplitude + wave_delta,
                            gravity: solver.gravity + wave_delta * 2.0,
                            ..solver
                        },
                    });
                }
            }
            PromptIntent::Unknown { .. } => {}
        }
    }

    edits
}

/// Applies graph edits and returns the updated graph.
pub fn apply_edits(graph: &Graph, edits: &[GraphEdit]) -> Graph {
    let mut result = graph.clone();

    for edit in edits {
        match edit {
            GraphEdit::LoadPreset { preset_id } => {
                if preset_id == "shop_street" {
                    result = Graph::shop_street_preset();
                } else if preset_id == "grid_block" {
                    result = Graph::grid_block_preset();
                } else if preset_id == "smoke_puff" {
                    result = Graph::smoke_puff_preset();
                } else if preset_id == "ocean_patch" {
                    result = Graph::ocean_patch_preset();
                } else if preset_id == "waterfall" {
                    result = Graph::waterfall_preset();
                } else if preset_id == "flood_basin" {
                    result = Graph::flood_basin_preset();
                }
            }
            GraphEdit::SetBuildingParams { node_id, params } => {
                for node in &mut result.nodes {
                    if node.id.0 == *node_id {
                        node.building_params = Some(*params);
                    }
                }
            }
            GraphEdit::SwitchPlacement { mode } => {
                result = switch_placement(&result, *mode);
            }
            GraphEdit::SetGraphName { name } => {
                result.name = name.clone();
            }
            GraphEdit::SetSmokeParams {
                source_node_id,
                source,
                solver_node_id,
                solver,
            } => {
                for node in &mut result.nodes {
                    if node.id.0 == *source_node_id {
                        node.smoke_source = Some(*source);
                    }
                    if node.id.0 == *solver_node_id {
                        node.smoke_solver = Some(*solver);
                    }
                }
            }
            GraphEdit::SetLiquidParams {
                source_node_id,
                source,
                solver_node_id,
                solver,
            } => {
                for node in &mut result.nodes {
                    if node.id.0 == *source_node_id {
                        node.liquid_source = Some(*source);
                    }
                    if node.id.0 == *solver_node_id {
                        node.liquid_solver = Some(*solver);
                    }
                }
            }
        }
    }

    result
}

/// Full prompt pipeline: interpret → edit → apply.
pub fn apply_prompt(graph: &Graph, text: &str) -> ApplyPromptResult {
    let intents = interpret_prompt(text);
    let edits = intents_to_edits(graph, &intents);
    let updated = apply_edits(graph, &edits);
    let summary = summarize_intents(&intents, &edits);
    ApplyPromptResult {
        graph: updated,
        intents,
        edits,
        summary,
    }
}

fn switch_placement(graph: &Graph, mode: PlacementMode) -> Graph {
    let mut g = graph.clone();
    let mesh_id = g
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::BuildingMesh)
        .map(|n| n.id.clone());
    let root_id = g
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::CityRoot)
        .map(|n| n.id.clone());
    let params_id = g
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::BuildingParams)
        .map(|n| n.id.clone());

    if mesh_id.is_none() || root_id.is_none() {
        return g;
    }
    let mesh_id = mesh_id.unwrap();
    let root_id = root_id.unwrap();

    g.edges.retain(|e| e.to != root_id && e.from != root_id);
    g.nodes.retain(|n| {
        n.kind != NodeKind::PlaceAlongPath && n.kind != NodeKind::FillGrid
    });

    match mode {
        PlacementMode::AlongPath => {
            let place_id = NodeId("place_along_path".into());
            g.nodes.push(crate::graph::Node {
                id: place_id.clone(),
                kind: NodeKind::PlaceAlongPath,
                label: "Street Placement".into(),
                building_params: None,
                path_input: Some(crate::placement::PathInput {
                    start: crate::mesh::Vec3::new(0.0, 0.0, -4.0),
                    end: crate::mesh::Vec3::new(48.0, 0.0, -4.0),
                    spacing: 9.0,
                    offset_from_path: 0.0,
                }),
                grid_input: None,
                smoke_domain: None,
                smoke_source: None,
                smoke_solver: None,
                liquid_domain: None,
                liquid_source: None,
                liquid_solver: None,
            });
            g.edges.push(crate::graph::Edge {
                from: mesh_id.clone(),
                to: place_id.clone(),
            });
            g.edges.push(crate::graph::Edge {
                from: place_id.clone(),
                to: root_id.clone(),
            });
            if let Some(pid) = params_id {
                g.edges.retain(|e| !(e.from == pid && e.to == mesh_id));
                if !g.edges.iter().any(|e| e.from == pid && e.to == mesh_id) {
                    g.edges.push(crate::graph::Edge {
                        from: pid,
                        to: mesh_id,
                    });
                }
            }
        }
        PlacementMode::Grid => {
            let grid_id = NodeId("fill_grid".into());
            g.nodes.push(crate::graph::Node {
                id: grid_id.clone(),
                kind: NodeKind::FillGrid,
                label: "Grid Fill".into(),
                building_params: None,
                path_input: None,
                grid_input: Some(GridInput::default()),
                smoke_domain: None,
                smoke_source: None,
                smoke_solver: None,
                liquid_domain: None,
                liquid_source: None,
                liquid_solver: None,
            });
            g.edges.push(crate::graph::Edge {
                from: mesh_id.clone(),
                to: grid_id.clone(),
            });
            g.edges.push(crate::graph::Edge {
                from: grid_id.clone(),
                to: root_id.clone(),
            });
            if let Some(pid) = params_id {
                g.edges.retain(|e| !(e.from == pid && e.to == mesh_id));
                if !g.edges.iter().any(|e| e.from == pid && e.to == mesh_id) {
                    g.edges.push(crate::graph::Edge {
                        from: pid,
                        to: mesh_id,
                    });
                }
            }
        }
    }

    g
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

fn parse_floors(text: &str) -> Option<u32> {
    // Japanese: 階数を5に, 5階
    if let Some(idx) = text.find('階') {
        let before = text[..idx].trim();
        if let Ok(n) = before
            .rsplit(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap_or("")
            .parse::<u32>()
        {
            if n > 0 && n <= 64 {
                return Some(n);
            }
        }
    }
    if text.contains("階数") {
        for word in text.split_whitespace() {
            if let Ok(n) = word.trim_matches(|c: char| !c.is_ascii_digit()).parse::<u32>() {
                if n > 0 && n <= 64 {
                    return Some(n);
                }
            }
        }
    }
    // English: "5 floors", "floors 5"
    let lower = text.to_lowercase();
    for token in lower.split_whitespace() {
        let digits = token.trim_matches(|c: char| !c.is_ascii_digit());
        if let Ok(n) = digits.parse::<u32>() {
            if n > 0 && n <= 64 {
                let rest = lower.replace(digits, "");
                if rest.contains("floor") || rest.contains("階") {
                    return Some(n);
                }
            }
        }
    }
    if lower.contains("floor") {
        for token in lower.split_whitespace() {
            if let Ok(n) = token.parse::<u32>() {
                if n > 0 && n <= 64 {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn pseudo_random_seed(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in text.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash.max(1)
}

fn summarize_intents(intents: &[PromptIntent], edits: &[GraphEdit]) -> String {
    if edits.is_empty() {
        if intents.iter().any(|i| matches!(i, PromptIntent::Unknown { .. })) {
            return "No matching local rules for that prompt.".into();
        }
        return "No changes applied.".into();
    }
    let parts: Vec<String> = edits
        .iter()
        .map(|e| match e {
            GraphEdit::LoadPreset { preset_id } => format!("Loaded preset {}", preset_id),
            GraphEdit::SetBuildingParams { params, .. } => {
                format!(
                    "Building params → {} floors, seed {}, windows {:.0}%",
                    params.floors,
                    params.seed,
                    params.window_density * 100.0
                )
            }
            GraphEdit::SwitchPlacement { mode } => format!("Placement → {}", mode_label(*mode)),
            GraphEdit::SetGraphName { name } => format!("Graph name → {}", name),
            GraphEdit::SetSmokeParams { source, solver, .. } => format!(
                "Smoke → emission {:.1}, {} steps",
                source.emission_rate, solver.steps
            ),
            GraphEdit::SetLiquidParams { source, solver, .. } => format!(
                "Liquid → emission {:.1}, wave {:.2}, {} steps",
                source.emission_rate, solver.wave_amplitude, solver.steps
            ),
        })
        .collect();
    parts.join("; ")
}

fn mode_label(mode: PlacementMode) -> &'static str {
    match mode {
        PlacementMode::AlongPath => "along path",
        PlacementMode::Grid => "grid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    #[test]
    fn interprets_shop_street_jp() {
        let intents = interpret_prompt("商店街にして");
        assert!(intents
            .iter()
            .any(|i| matches!(i, PromptIntent::LoadPreset { .. })));
    }

    #[test]
    fn interprets_floors() {
        let intents = interpret_prompt("階数を5に");
        assert!(intents
            .iter()
            .any(|i| matches!(i, PromptIntent::SetFloors { floors: 5 })));
    }

    #[test]
    fn applies_window_delta() {
        let graph = Graph::shop_street_preset();
        let result = apply_prompt(&graph, "もっと窓");
        let params = result
            .graph
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::BuildingParams)
            .and_then(|n| n.building_params)
            .expect("params");
        let orig = graph
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::BuildingParams)
            .and_then(|n| n.building_params)
            .expect("orig");
        assert!(params.window_density > orig.window_density);
    }

    #[test]
    fn interprets_smoke_jp() {
        let intents = interpret_prompt("煙のプレビュー");
        assert!(intents.iter().any(|i| matches!(
            i,
            PromptIntent::LoadPreset { preset_id } if preset_id == "smoke_puff"
        )));
    }

    #[test]
    fn interprets_ocean_jp() {
        let intents = interpret_prompt("海のプレビュー");
        assert!(intents.iter().any(|i| matches!(
            i,
            PromptIntent::LoadPreset { preset_id } if preset_id == "ocean_patch"
        )));
    }

    #[test]
    fn interprets_waterfall_en() {
        let intents = interpret_prompt("load waterfall preset");
        assert!(intents.iter().any(|i| matches!(
            i,
            PromptIntent::LoadPreset { preset_id } if preset_id == "waterfall"
        )));
    }

    #[test]
    fn interprets_flood_jp() {
        let intents = interpret_prompt("洪水シミュレーション");
        assert!(intents.iter().any(|i| matches!(
            i,
            PromptIntent::LoadPreset { preset_id } if preset_id == "flood_basin"
        )));
    }

    #[test]
    fn applies_more_liquid_intensity() {
        let graph = Graph::waterfall_preset();
        let result = apply_prompt(&graph, "もっと激しく");
        let solver = result
            .graph
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::LiquidSolver)
            .and_then(|n| n.liquid_solver)
            .expect("solver");
        let orig = graph
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::LiquidSolver)
            .and_then(|n| n.liquid_solver)
            .expect("orig");
        assert!(solver.steps > orig.steps || solver.gravity > orig.gravity);
    }

    #[test]
    fn applies_more_smoke() {
        let graph = Graph::smoke_puff_preset();
        let result = apply_prompt(&graph, "もっと煙");
        let solver = result
            .graph
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::SmokeSolver)
            .and_then(|n| n.smoke_solver)
            .expect("solver");
        let orig = graph
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::SmokeSolver)
            .and_then(|n| n.smoke_solver)
            .expect("orig");
        assert!(solver.steps > orig.steps);
    }
}
