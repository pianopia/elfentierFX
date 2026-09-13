//! Procedural node graph with cook/evaluate support.

use crate::building::{generate_building, BuildingParams};
use crate::export::export_glb;
use crate::mesh::{Mesh, Vec3};
use crate::placement::{
    fill_grid, place_along_path, GridInput, InstanceTransform, PathInput,
};
use crate::liquid::{
    simulate_liquid, LiquidDomainInput, LiquidSolverInput, LiquidSourceInput, LiquidVolume,
};
use crate::smoke::{
    simulate_smoke, SmokeDomainInput, SmokeSolverInput, SmokeSourceInput, SmokeVolume,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a node in the procedural graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Supported procedural node kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    BuildingParams,
    BuildingMesh,
    PlaceAlongPath,
    FillGrid,
    MergeInstances,
    CityRoot,
    SmokeDomain,
    SmokeSource,
    SmokeSolver,
    SmokeRoot,
    LiquidDomain,
    LiquidSource,
    LiquidSolver,
    LiquidRoot,
}

/// Graph cook mode inferred from the output root node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphMode {
    City,
    Smoke,
    Liquid,
}

/// Returns the cook mode for a graph document.
pub fn graph_mode(graph: &Graph) -> GraphMode {
    if graph.nodes.iter().any(|n| n.kind == NodeKind::LiquidRoot) {
        GraphMode::Liquid
    } else if graph.nodes.iter().any(|n| n.kind == NodeKind::SmokeRoot) {
        GraphMode::Smoke
    } else {
        GraphMode::City
    }
}

/// A node in the procedural graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub label: String,
    #[serde(default)]
    pub building_params: Option<BuildingParams>,
    #[serde(default)]
    pub path_input: Option<PathInput>,
    #[serde(default)]
    pub grid_input: Option<GridInput>,
    #[serde(default)]
    pub smoke_domain: Option<SmokeDomainInput>,
    #[serde(default)]
    pub smoke_source: Option<SmokeSourceInput>,
    #[serde(default)]
    pub smoke_solver: Option<SmokeSolverInput>,
    #[serde(default)]
    pub liquid_domain: Option<LiquidDomainInput>,
    #[serde(default)]
    pub liquid_source: Option<LiquidSourceInput>,
    #[serde(default)]
    pub liquid_solver: Option<LiquidSolverInput>,
}

/// Edge connecting an output port to an input port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
}

/// Serializable graph document exchanged with the UI host.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    pub name: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn grid_block_preset() -> Self {
        let params_id = NodeId("building_params".into());
        let mesh_id = NodeId("building_mesh".into());
        let grid_id = NodeId("fill_grid".into());
        let root_id = NodeId("city_root".into());

        Self {
            name: "Grid Block".into(),
            nodes: vec![
                empty_node(params_id.clone(), NodeKind::BuildingParams, "Block Params")
                    .with_building_params(BuildingParams::default()),
                empty_node(mesh_id.clone(), NodeKind::BuildingMesh, "Building Mesh"),
                empty_node(grid_id.clone(), NodeKind::FillGrid, "Grid Fill")
                    .with_grid_input(GridInput::default()),
                empty_node(root_id.clone(), NodeKind::CityRoot, "City Root"),
            ],
            edges: vec![
                Edge { from: params_id.clone(), to: mesh_id.clone() },
                Edge { from: mesh_id.clone(), to: grid_id.clone() },
                Edge { from: grid_id.clone(), to: root_id.clone() },
            ],
        }
    }

    pub fn shop_street_preset() -> Self {
        let params_id = NodeId("building_params".into());
        let mesh_id = NodeId("building_mesh".into());
        let place_id = NodeId("place_along_path".into());
        let root_id = NodeId("city_root".into());

        Self {
            name: "Shop Street".into(),
            nodes: vec![
                empty_node(params_id.clone(), NodeKind::BuildingParams, "Shop Params")
                    .with_building_params(BuildingParams::shop_preset()),
                empty_node(mesh_id.clone(), NodeKind::BuildingMesh, "Building Mesh"),
                empty_node(place_id.clone(), NodeKind::PlaceAlongPath, "Street Placement")
                    .with_path_input(PathInput {
                        start: Vec3::new(0.0, 0.0, -4.0),
                        end: Vec3::new(48.0, 0.0, -4.0),
                        spacing: 9.0,
                        offset_from_path: 0.0,
                    }),
                empty_node(root_id.clone(), NodeKind::CityRoot, "City Root"),
            ],
            edges: vec![
                Edge { from: params_id.clone(), to: mesh_id.clone() },
                Edge { from: mesh_id.clone(), to: place_id.clone() },
                Edge { from: place_id.clone(), to: root_id.clone() },
            ],
        }
    }

    /// Standalone smoke puff preset — no city geometry required.
    pub fn smoke_puff_preset() -> Self {
        let domain_id = NodeId("smoke_domain".into());
        let source_id = NodeId("smoke_source".into());
        let solver_id = NodeId("smoke_solver".into());
        let root_id = NodeId("smoke_root".into());

        Self {
            name: "Smoke Puff".into(),
            nodes: vec![
                empty_node(domain_id.clone(), NodeKind::SmokeDomain, "Smoke Domain")
                    .with_smoke_domain(SmokeDomainInput::default()),
                empty_node(source_id.clone(), NodeKind::SmokeSource, "Puff Source")
                    .with_smoke_source(SmokeSourceInput::default()),
                empty_node(solver_id.clone(), NodeKind::SmokeSolver, "Smoke Solver")
                    .with_smoke_solver(SmokeSolverInput::default()),
                empty_node(root_id.clone(), NodeKind::SmokeRoot, "Smoke Root"),
            ],
            edges: vec![
                Edge { from: domain_id.clone(), to: source_id.clone() },
                Edge { from: source_id.clone(), to: solver_id.clone() },
                Edge { from: solver_id.clone(), to: root_id.clone() },
            ],
        }
    }

    /// Wide ocean surface with gentle wave forcing.
    pub fn ocean_patch_preset() -> Self {
        let domain_id = NodeId("liquid_domain".into());
        let source_id = NodeId("liquid_source".into());
        let solver_id = NodeId("liquid_solver".into());
        let root_id = NodeId("liquid_root".into());

        Self {
            name: "Ocean Patch".into(),
            nodes: vec![
                empty_node(domain_id.clone(), NodeKind::LiquidDomain, "Liquid Domain")
                    .with_liquid_domain(LiquidDomainInput {
                        resolution: 22,
                        bounds_min: Vec3::new(-8.0, 0.0, -8.0),
                        bounds_max: Vec3::new(8.0, 3.5, 8.0),
                        seed: 21,
                        initial_particles: 2400,
                        particle_radius: 0.14,
                    }),
                empty_node(source_id.clone(), NodeKind::LiquidSource, "Surface Fill")
                    .with_liquid_source(LiquidSourceInput {
                        position: Vec3::new(0.0, 2.8, 0.0),
                        radius: 7.0,
                        emission_rate: 0.0,
                        velocity: Vec3::new(0.0, 0.0, 0.0),
                        active_until_step: 0,
                    }),
                empty_node(solver_id.clone(), NodeKind::LiquidSolver, "Liquid Solver")
                    .with_liquid_solver(LiquidSolverInput {
                        steps: 48,
                        frame_stride: 4,
                        gravity: 4.5,
                        flip_ratio: 0.94,
                        viscosity: 0.04,
                        pressure_iterations: 18,
                        wave_amplitude: 0.28,
                        wave_frequency: 1.1,
                        terrain_height: 0.0,
                        max_particles: 12000,
                    }),
                empty_node(root_id.clone(), NodeKind::LiquidRoot, "Liquid Surface"),
            ],
            edges: vec![
                Edge { from: domain_id.clone(), to: source_id.clone() },
                Edge { from: source_id.clone(), to: solver_id.clone() },
                Edge { from: solver_id.clone(), to: root_id.clone() },
            ],
        }
    }

    /// Elevated source with gravity-driven fall and splashy landing.
    pub fn waterfall_preset() -> Self {
        let domain_id = NodeId("liquid_domain".into());
        let source_id = NodeId("liquid_source".into());
        let solver_id = NodeId("liquid_solver".into());
        let root_id = NodeId("liquid_root".into());

        Self {
            name: "Waterfall".into(),
            nodes: vec![
                empty_node(domain_id.clone(), NodeKind::LiquidDomain, "Liquid Domain")
                    .with_liquid_domain(LiquidDomainInput {
                        resolution: 20,
                        bounds_min: Vec3::new(-3.0, 0.0, -3.0),
                        bounds_max: Vec3::new(3.0, 10.0, 3.0),
                        seed: 33,
                        initial_particles: 200,
                        particle_radius: 0.1,
                    }),
                empty_node(source_id.clone(), NodeKind::LiquidSource, "Cascade Source")
                    .with_liquid_source(LiquidSourceInput {
                        position: Vec3::new(0.0, 9.0, 0.0),
                        radius: 0.7,
                        emission_rate: 12.0,
                        velocity: Vec3::new(0.0, -3.5, 0.0),
                        active_until_step: 0,
                    }),
                empty_node(solver_id.clone(), NodeKind::LiquidSolver, "Liquid Solver")
                    .with_liquid_solver(LiquidSolverInput {
                        steps: 72,
                        frame_stride: 3,
                        gravity: 12.0,
                        flip_ratio: 0.97,
                        viscosity: 0.01,
                        pressure_iterations: 22,
                        wave_amplitude: 0.0,
                        wave_frequency: 0.0,
                        terrain_height: 0.0,
                        max_particles: 14000,
                    }),
                empty_node(root_id.clone(), NodeKind::LiquidRoot, "Liquid Surface"),
            ],
            edges: vec![
                Edge { from: domain_id.clone(), to: source_id.clone() },
                Edge { from: source_id.clone(), to: solver_id.clone() },
                Edge { from: solver_id.clone(), to: root_id.clone() },
            ],
        }
    }

    /// Basin fill from a source over time.
    pub fn flood_basin_preset() -> Self {
        let domain_id = NodeId("liquid_domain".into());
        let source_id = NodeId("liquid_source".into());
        let solver_id = NodeId("liquid_solver".into());
        let root_id = NodeId("liquid_root".into());

        Self {
            name: "Flood Basin".into(),
            nodes: vec![
                empty_node(domain_id.clone(), NodeKind::LiquidDomain, "Liquid Domain")
                    .with_liquid_domain(LiquidDomainInput {
                        resolution: 20,
                        bounds_min: Vec3::new(-5.0, 0.0, -5.0),
                        bounds_max: Vec3::new(5.0, 4.0, 5.0),
                        seed: 55,
                        initial_particles: 100,
                        particle_radius: 0.11,
                    }),
                empty_node(source_id.clone(), NodeKind::LiquidSource, "Inflow Source")
                    .with_liquid_source(LiquidSourceInput {
                        position: Vec3::new(-3.5, 0.6, 0.0),
                        radius: 0.5,
                        emission_rate: 10.0,
                        velocity: Vec3::new(1.2, 0.3, 0.0),
                        active_until_step: 80,
                    }),
                empty_node(solver_id.clone(), NodeKind::LiquidSolver, "Liquid Solver")
                    .with_liquid_solver(LiquidSolverInput {
                        steps: 90,
                        frame_stride: 3,
                        gravity: 9.8,
                        flip_ratio: 0.95,
                        viscosity: 0.03,
                        pressure_iterations: 20,
                        wave_amplitude: 0.0,
                        wave_frequency: 0.0,
                        terrain_height: 0.0,
                        max_particles: 16000,
                    }),
                empty_node(root_id.clone(), NodeKind::LiquidRoot, "Liquid Surface"),
            ],
            edges: vec![
                Edge { from: domain_id.clone(), to: source_id.clone() },
                Edge { from: source_id.clone(), to: solver_id.clone() },
                Edge { from: solver_id.clone(), to: root_id.clone() },
            ],
        }
    }
}

fn empty_node(id: NodeId, kind: NodeKind, label: &str) -> Node {
    Node {
        id,
        kind,
        label: label.into(),
        building_params: None,
        path_input: None,
        grid_input: None,
        smoke_domain: None,
        smoke_source: None,
        smoke_solver: None,
        liquid_domain: None,
        liquid_source: None,
        liquid_solver: None,
    }
}

impl Node {
    fn with_building_params(mut self, params: BuildingParams) -> Self {
        self.building_params = Some(params);
        self
    }

    fn with_path_input(mut self, path: PathInput) -> Self {
        self.path_input = Some(path);
        self
    }

    fn with_grid_input(mut self, grid: GridInput) -> Self {
        self.grid_input = Some(grid);
        self
    }

    fn with_smoke_domain(mut self, domain: SmokeDomainInput) -> Self {
        self.smoke_domain = Some(domain);
        self
    }

    fn with_smoke_source(mut self, source: SmokeSourceInput) -> Self {
        self.smoke_source = Some(source);
        self
    }

    fn with_smoke_solver(mut self, solver: SmokeSolverInput) -> Self {
        self.smoke_solver = Some(solver);
        self
    }

    fn with_liquid_domain(mut self, domain: LiquidDomainInput) -> Self {
        self.liquid_domain = Some(domain);
        self
    }

    fn with_liquid_source(mut self, source: LiquidSourceInput) -> Self {
        self.liquid_source = Some(source);
        self
    }

    fn with_liquid_solver(mut self, solver: LiquidSolverInput) -> Self {
        self.liquid_solver = Some(solver);
        self
    }
}

/// Cooked output statistics for city and smoke graphs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CookResult {
    pub vertex_count: u32,
    pub index_count: u32,
    pub triangle_count: u32,
    pub instance_count: u32,
    pub graph_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoke_resolution: Option<[u32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoke_max_density: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoke_steps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoke_frame_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoke_particle_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid_resolution: Option<[u32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid_steps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid_frame_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid_particle_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid_max_speed: Option<f32>,
}

/// Internal evaluated values flowing through the graph.
#[derive(Debug, Clone)]
enum NodeValue {
    Params(BuildingParams),
    Mesh(Mesh),
    Instances(Vec<InstanceTransform>),
    City {
        base_mesh: Mesh,
        instances: Vec<InstanceTransform>,
    },
    SmokeSetup {
        domain: SmokeDomainInput,
        sources: Vec<SmokeSourceInput>,
        solver: SmokeSolverInput,
    },
    SmokeVolume(SmokeVolume),
    LiquidSetup {
        domain: LiquidDomainInput,
        sources: Vec<LiquidSourceInput>,
        solver: LiquidSolverInput,
    },
    LiquidVolume(LiquidVolume),
}

/// Evaluated city geometry ready for stats or export.
#[derive(Debug, Clone)]
pub struct CityOutput {
    pub mesh: Mesh,
    pub instances: Vec<InstanceTransform>,
}

/// Instanced city geometry (base mesh + transforms) for viewport rendering.
#[derive(Debug, Clone)]
pub struct CityInstanced {
    pub base_mesh: Mesh,
    pub instances: Vec<InstanceTransform>,
}

/// Evaluates the graph and returns combined mesh or smoke statistics.
pub fn cook_graph(graph: &Graph) -> Result<CookResult, String> {
    match graph_mode(graph) {
        GraphMode::Liquid => {
            let volume = evaluate_liquid_volume(graph)?;
            Ok(CookResult {
                vertex_count: 0,
                index_count: 0,
                triangle_count: 0,
                instance_count: 0,
                graph_name: graph.name.clone(),
                smoke_resolution: None,
                smoke_max_density: None,
                smoke_steps: None,
                smoke_frame_count: None,
                smoke_particle_count: None,
                liquid_resolution: Some(volume.stats.resolution),
                liquid_steps: Some(volume.stats.step_count),
                liquid_frame_count: Some(volume.stats.frame_count),
                liquid_particle_count: Some(volume.stats.particle_count),
                liquid_max_speed: Some(volume.stats.max_speed),
            })
        }
        GraphMode::Smoke => {
            let volume = evaluate_smoke_volume(graph)?;
            Ok(CookResult {
                vertex_count: 0,
                index_count: 0,
                triangle_count: 0,
                instance_count: 0,
                graph_name: graph.name.clone(),
                smoke_resolution: Some(volume.stats.resolution),
                smoke_max_density: Some(volume.stats.max_density),
                smoke_steps: Some(volume.stats.step_count),
                smoke_frame_count: Some(volume.stats.frame_count),
                smoke_particle_count: Some(volume.stats.particle_count),
                liquid_resolution: None,
                liquid_steps: None,
                liquid_frame_count: None,
                liquid_particle_count: None,
                liquid_max_speed: None,
            })
        }
        GraphMode::City => {
            let city = evaluate_city(graph)?;
            Ok(CookResult {
                vertex_count: city.mesh.vertex_count(),
                index_count: city.mesh.index_count(),
                triangle_count: city.mesh.triangle_count(),
                instance_count: city.instances.len() as u32,
                graph_name: graph.name.clone(),
                smoke_resolution: None,
                smoke_max_density: None,
                smoke_steps: None,
                smoke_frame_count: None,
                smoke_particle_count: None,
                liquid_resolution: None,
                liquid_steps: None,
                liquid_frame_count: None,
                liquid_particle_count: None,
                liquid_max_speed: None,
            })
        }
    }
}

/// Evaluates the graph and returns base mesh + instance transforms (no merge).
pub fn evaluate_city_instanced(graph: &Graph) -> Result<CityInstanced, String> {
    let root = graph
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::CityRoot)
        .ok_or("graph missing CityRoot node")?;

    let values = evaluate_all(graph)?;
    match values.get(&root.id) {
        Some(NodeValue::City { base_mesh, instances, .. }) => Ok(CityInstanced {
            base_mesh: base_mesh.clone(),
            instances: instances.clone(),
        }),
        Some(NodeValue::Mesh(m)) => Ok(CityInstanced {
            base_mesh: m.clone(),
            instances: vec![InstanceTransform::default()],
        }),
        Some(_) => Err("CityRoot did not receive city output".into()),
        None => Err("CityRoot was not evaluated".into()),
    }
}

/// Evaluates a liquid graph and returns the simulated volume.
pub fn evaluate_liquid_volume(graph: &Graph) -> Result<LiquidVolume, String> {
    let root = graph
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::LiquidRoot)
        .ok_or("graph missing LiquidRoot node")?;

    let values = evaluate_all(graph)?;
    match values.get(&root.id) {
        Some(NodeValue::LiquidVolume(v)) => Ok(v.clone()),
        Some(_) => Err("LiquidRoot did not receive liquid volume".into()),
        None => Err("LiquidRoot was not evaluated".into()),
    }
}

/// Evaluates a smoke graph and returns the simulated volume.
pub fn evaluate_smoke_volume(graph: &Graph) -> Result<SmokeVolume, String> {
    let root = graph
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::SmokeRoot)
        .ok_or("graph missing SmokeRoot node")?;

    let values = evaluate_all(graph)?;
    match values.get(&root.id) {
        Some(NodeValue::SmokeVolume(v)) => Ok(v.clone()),
        Some(_) => Err("SmokeRoot did not receive smoke volume".into()),
        None => Err("SmokeRoot was not evaluated".into()),
    }
}

/// Evaluates the graph, merges instanced geometry, and exports glTF.
pub fn cook_and_export(graph: &Graph, path: &str) -> Result<crate::export::ExportResult, String> {
    match graph_mode(graph) {
        GraphMode::Smoke => Err(
            "glTF export is for city mesh graphs; use export_smoke_density for smoke".into(),
        ),
        GraphMode::Liquid => Err(
            "glTF export is for city mesh graphs; use export_liquid_cache for liquid".into(),
        ),
        GraphMode::City => {
            let city = evaluate_city(graph)?;
            export_glb(&city.mesh, path).map_err(|e| e.to_string())
        }
    }
}

fn evaluate_city(graph: &Graph) -> Result<CityOutput, String> {
    let instanced = evaluate_city_instanced(graph)?;
    let merged = merge_instances(&instanced.base_mesh, &instanced.instances);
    Ok(CityOutput {
        mesh: merged,
        instances: instanced.instances,
    })
}

fn evaluate_all(graph: &Graph) -> Result<HashMap<NodeId, NodeValue>, String> {
    let mut values: HashMap<NodeId, NodeValue> = HashMap::new();
    let incoming: HashMap<NodeId, Vec<NodeId>> = graph
        .edges
        .iter()
        .fold(HashMap::new(), |mut map, edge| {
            map.entry(edge.to.clone()).or_default().push(edge.from.clone());
            map
        });

    for node in &graph.nodes {
        let inputs = incoming.get(&node.id).cloned().unwrap_or_default();
        let value = evaluate_node(node, &inputs, &values)?;
        values.insert(node.id.clone(), value);
    }
    Ok(values)
}

fn evaluate_node(
    node: &Node,
    inputs: &[NodeId],
    cache: &HashMap<NodeId, NodeValue>,
) -> Result<NodeValue, String> {
    match node.kind {
        NodeKind::BuildingParams => {
            let params = node
                .building_params
                .clone()
                .ok_or_else(|| format!("{} missing building_params", node.id.0))?;
            Ok(NodeValue::Params(params))
        }
        NodeKind::BuildingMesh => {
            let params = input_params(inputs, cache)?;
            let mesh = generate_building(&params);
            Ok(NodeValue::Mesh(mesh))
        }
        NodeKind::PlaceAlongPath => {
            let mesh = input_mesh(inputs, cache)?;
            let path = node.path_input.clone().unwrap_or_default();
            let seed = node.building_params.map(|p| p.seed).unwrap_or(1);
            let instances = place_along_path(&path, seed);
            Ok(NodeValue::City { base_mesh: mesh, instances })
        }
        NodeKind::FillGrid => {
            let mesh = input_mesh(inputs, cache)?;
            let grid = node.grid_input.clone().unwrap_or_default();
            let seed = node.building_params.map(|p| p.seed).unwrap_or(1);
            let instances = fill_grid(&grid, seed);
            Ok(NodeValue::City { base_mesh: mesh, instances })
        }
        NodeKind::MergeInstances => {
            let mut all_instances = Vec::new();
            let mut base_mesh: Option<Mesh> = None;
            for input_id in inputs {
                match cache.get(input_id) {
                    Some(NodeValue::City { base_mesh: bm, instances }) => {
                        if base_mesh.is_none() {
                            base_mesh = Some(bm.clone());
                        }
                        all_instances.extend(instances.clone());
                    }
                    Some(NodeValue::Instances(list)) => all_instances.extend(list.clone()),
                    Some(NodeValue::Mesh(m)) => {
                        if base_mesh.is_none() {
                            base_mesh = Some(m.clone());
                        }
                    }
                    _ => {}
                }
            }
            let mesh = base_mesh.unwrap_or_else(Mesh::unnamed);
            Ok(NodeValue::City {
                base_mesh: mesh,
                instances: all_instances,
            })
        }
        NodeKind::CityRoot => {
            let input_id = inputs.first().ok_or("CityRoot requires an input edge")?;
            match cache.get(input_id) {
                Some(NodeValue::City { base_mesh, instances }) => Ok(NodeValue::City {
                    base_mesh: base_mesh.clone(),
                    instances: instances.clone(),
                }),
                Some(NodeValue::Mesh(mesh)) => Ok(NodeValue::City {
                    base_mesh: mesh.clone(),
                    instances: vec![InstanceTransform::default()],
                }),
                _ => Err(format!("CityRoot input {} has unsupported type", input_id.0)),
            }
        }
        NodeKind::SmokeDomain => {
            let domain = node
                .smoke_domain
                .clone()
                .ok_or_else(|| format!("{} missing smoke_domain", node.id.0))?;
            Ok(NodeValue::SmokeSetup {
                domain,
                sources: Vec::new(),
                solver: SmokeSolverInput::default(),
            })
        }
        NodeKind::SmokeSource => {
            let mut setup = input_smoke_setup(inputs, cache)?;
            if let Some(source) = node.smoke_source.clone() {
                setup.sources.push(source);
            }
            Ok(NodeValue::SmokeSetup {
                domain: setup.domain,
                sources: setup.sources,
                solver: setup.solver,
            })
        }
        NodeKind::SmokeSolver => {
            let setup = input_smoke_setup(inputs, cache)?;
            let solver = node.smoke_solver.clone().unwrap_or(setup.solver);
            let volume = simulate_smoke(&setup.domain, &setup.sources, &solver);
            Ok(NodeValue::SmokeVolume(volume))
        }
        NodeKind::SmokeRoot => {
            let input_id = inputs.first().ok_or("SmokeRoot requires an input edge")?;
            match cache.get(input_id) {
                Some(NodeValue::SmokeVolume(v)) => Ok(NodeValue::SmokeVolume(v.clone())),
                _ => Err(format!("SmokeRoot input {} has unsupported type", input_id.0)),
            }
        }
        NodeKind::LiquidDomain => {
            let domain = node
                .liquid_domain
                .clone()
                .ok_or_else(|| format!("{} missing liquid_domain", node.id.0))?;
            Ok(NodeValue::LiquidSetup {
                domain,
                sources: Vec::new(),
                solver: LiquidSolverInput::default(),
            })
        }
        NodeKind::LiquidSource => {
            let mut setup = input_liquid_setup(inputs, cache)?;
            if let Some(source) = node.liquid_source.clone() {
                setup.sources.push(source);
            }
            Ok(NodeValue::LiquidSetup {
                domain: setup.domain,
                sources: setup.sources,
                solver: setup.solver,
            })
        }
        NodeKind::LiquidSolver => {
            let setup = input_liquid_setup(inputs, cache)?;
            let solver = node.liquid_solver.clone().unwrap_or(setup.solver);
            let volume = simulate_liquid(&setup.domain, &setup.sources, &solver);
            Ok(NodeValue::LiquidVolume(volume))
        }
        NodeKind::LiquidRoot => {
            let input_id = inputs.first().ok_or("LiquidRoot requires an input edge")?;
            match cache.get(input_id) {
                Some(NodeValue::LiquidVolume(v)) => Ok(NodeValue::LiquidVolume(v.clone())),
                _ => Err(format!("LiquidRoot input {} has unsupported type", input_id.0)),
            }
        }
    }
}

fn input_params(inputs: &[NodeId], cache: &HashMap<NodeId, NodeValue>) -> Result<BuildingParams, String> {
    for id in inputs {
        if let Some(NodeValue::Params(p)) = cache.get(id) {
            return Ok(*p);
        }
    }
    Err("BuildingMesh missing BuildingParams input".into())
}

fn input_mesh(inputs: &[NodeId], cache: &HashMap<NodeId, NodeValue>) -> Result<Mesh, String> {
    for id in inputs {
        if let Some(NodeValue::Mesh(m)) = cache.get(id) {
            return Ok(m.clone());
        }
    }
    Err("placement node missing BuildingMesh input".into())
}

struct SmokeSetupAccum {
    domain: SmokeDomainInput,
    sources: Vec<SmokeSourceInput>,
    solver: SmokeSolverInput,
}

struct LiquidSetupAccum {
    domain: LiquidDomainInput,
    sources: Vec<LiquidSourceInput>,
    solver: LiquidSolverInput,
}

fn input_liquid_setup(
    inputs: &[NodeId],
    cache: &HashMap<NodeId, NodeValue>,
) -> Result<LiquidSetupAccum, String> {
    for id in inputs {
        if let Some(NodeValue::LiquidSetup { domain, sources, solver }) = cache.get(id) {
            return Ok(LiquidSetupAccum {
                domain: *domain,
                sources: sources.clone(),
                solver: *solver,
            });
        }
    }
    Err("liquid node missing LiquidDomain upstream chain".into())
}

fn input_smoke_setup(
    inputs: &[NodeId],
    cache: &HashMap<NodeId, NodeValue>,
) -> Result<SmokeSetupAccum, String> {
    for id in inputs {
        if let Some(NodeValue::SmokeSetup { domain, sources, solver }) = cache.get(id) {
            return Ok(SmokeSetupAccum {
                domain: *domain,
                sources: sources.clone(),
                solver: *solver,
            });
        }
    }
    Err("smoke node missing SmokeDomain upstream chain".into())
}

fn merge_instances(base: &Mesh, instances: &[InstanceTransform]) -> Mesh {
    let mut merged = Mesh::with_name("city_block");
    if instances.is_empty() {
        merged.merge(base);
        return merged;
    }
    for inst in instances {
        let transformed = base.transform(inst.position, inst.rotation_y, inst.scale);
        merged.merge(&transformed);
    }
    merged.vertices.byte_len = merged.positions.len() * 12;
    merged.indices_buffer.byte_len = merged.indices.len() * 4;
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_cooks_street() {
        let graph = Graph::shop_street_preset();
        let result = cook_graph(&graph).expect("cook");
        assert!(result.instance_count >= 4);
        assert!(result.triangle_count > 12);
    }

    #[test]
    fn smoke_preset_cooks() {
        let graph = Graph::smoke_puff_preset();
        let result = cook_graph(&graph).expect("cook smoke");
        assert_eq!(result.vertex_count, 0);
        assert!(result.smoke_max_density.unwrap_or(0.0) > 0.0);
        assert!(result.smoke_steps.unwrap_or(0) >= 1);
        assert!(result.smoke_particle_count.unwrap_or(0) > 0);
    }

    #[test]
    fn liquid_ocean_preset_cooks() {
        let graph = Graph::ocean_patch_preset();
        let result = cook_graph(&graph).expect("cook liquid");
        assert!(result.liquid_particle_count.unwrap_or(0) > 0);
        assert!(result.liquid_steps.unwrap_or(0) >= 1);
    }

    #[test]
    fn liquid_waterfall_preset_cooks() {
        let graph = Graph::waterfall_preset();
        let result = cook_graph(&graph).expect("cook waterfall");
        assert!(result.liquid_particle_count.unwrap_or(0) > 0);
    }

    #[test]
    fn liquid_flood_preset_cooks() {
        let graph = Graph::flood_basin_preset();
        let result = cook_graph(&graph).expect("cook flood");
        assert!(result.liquid_frame_count.unwrap_or(0) >= 2);
    }

    #[test]
    fn grid_graph_cooks() {
        let params_id = NodeId("params".into());
        let mesh_id = NodeId("mesh".into());
        let grid_id = NodeId("grid".into());
        let root_id = NodeId("root".into());
        let graph = Graph {
            name: "Grid Block".into(),
            nodes: vec![
                empty_node(params_id.clone(), NodeKind::BuildingParams, "Params")
                    .with_building_params(BuildingParams::default()),
                empty_node(mesh_id.clone(), NodeKind::BuildingMesh, "Mesh"),
                empty_node(grid_id.clone(), NodeKind::FillGrid, "Grid")
                    .with_grid_input(GridInput::default()),
                empty_node(root_id.clone(), NodeKind::CityRoot, "Root"),
            ],
            edges: vec![
                Edge { from: params_id.clone(), to: mesh_id.clone() },
                Edge { from: mesh_id.clone(), to: grid_id.clone() },
                Edge { from: grid_id.clone(), to: root_id.clone() },
            ],
        };
        let result = cook_graph(&graph).expect("cook");
        assert_eq!(result.instance_count, 6);
    }
}
