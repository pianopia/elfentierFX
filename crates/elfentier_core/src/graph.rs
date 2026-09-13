//! Procedural node graph with cook/evaluate support.

use crate::building::{generate_building, BuildingParams};
use crate::export::export_glb;
use crate::mesh::{Mesh, Vec3};
use crate::placement::{
    fill_grid, place_along_path, GridInput, InstanceTransform, PathInput,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a node in the procedural graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Supported procedural node kinds for Alpha 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    BuildingParams,
    BuildingMesh,
    PlaceAlongPath,
    FillGrid,
    MergeInstances,
    CityRoot,
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

    pub fn shop_street_preset() -> Self {
        let params_id = NodeId("building_params".into());
        let mesh_id = NodeId("building_mesh".into());
        let place_id = NodeId("place_along_path".into());
        let root_id = NodeId("city_root".into());

        Self {
            name: "Shop Street".into(),
            nodes: vec![
                Node {
                    id: params_id.clone(),
                    kind: NodeKind::BuildingParams,
                    label: "Shop Params".into(),
                    building_params: Some(BuildingParams::shop_preset()),
                    path_input: None,
                    grid_input: None,
                },
                Node {
                    id: mesh_id.clone(),
                    kind: NodeKind::BuildingMesh,
                    label: "Building Mesh".into(),
                    building_params: None,
                    path_input: None,
                    grid_input: None,
                },
                Node {
                    id: place_id.clone(),
                    kind: NodeKind::PlaceAlongPath,
                    label: "Street Placement".into(),
                    building_params: None,
                    path_input: Some(PathInput {
                        start: Vec3::new(0.0, 0.0, -4.0),
                        end: Vec3::new(48.0, 0.0, -4.0),
                        spacing: 9.0,
                        offset_from_path: 0.0,
                    }),
                    grid_input: None,
                },
                Node {
                    id: root_id.clone(),
                    kind: NodeKind::CityRoot,
                    label: "City Root".into(),
                    building_params: None,
                    path_input: None,
                    grid_input: None,
                },
            ],
            edges: vec![
                Edge {
                    from: params_id.clone(),
                    to: mesh_id.clone(),
                },
                Edge {
                    from: mesh_id.clone(),
                    to: place_id.clone(),
                },
                Edge {
                    from: place_id.clone(),
                    to: root_id.clone(),
                },
            ],
        }
    }
}

/// Cooked city output with mesh stats and instance metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CookResult {
    pub vertex_count: u32,
    pub index_count: u32,
    pub triangle_count: u32,
    pub instance_count: u32,
    pub graph_name: String,
}

/// Internal evaluated values flowing through the graph.
#[derive(Debug, Clone)]
enum NodeValue {
    Params(BuildingParams),
    Mesh(Mesh),
    Instances(Vec<InstanceTransform>), // reserved for multi-merge inputs
    City {
        mesh: Mesh,
        instances: Vec<InstanceTransform>,
    },
}

/// Evaluated city geometry ready for stats or export.
#[derive(Debug, Clone)]
pub struct CityOutput {
    pub mesh: Mesh,
    pub instances: Vec<InstanceTransform>,
}

/// Evaluates the graph and returns combined mesh statistics.
pub fn cook_graph(graph: &Graph) -> Result<CookResult, String> {
    let city = evaluate_city(graph)?;
    Ok(CookResult {
        vertex_count: city.mesh.vertex_count(),
        index_count: city.mesh.index_count(),
        triangle_count: city.mesh.triangle_count(),
        instance_count: city.instances.len() as u32,
        graph_name: graph.name.clone(),
    })
}

/// Evaluates the graph, merges instanced geometry, and exports glTF.
pub fn cook_and_export(graph: &Graph, path: &str) -> Result<crate::export::ExportResult, String> {
    let city = evaluate_city(graph)?;
    export_glb(&city.mesh, path).map_err(|e| e.to_string())
}

fn evaluate_city(graph: &Graph) -> Result<CityOutput, String> {
    let root = graph
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::CityRoot)
        .ok_or("graph missing CityRoot node")?;

    let values = evaluate_all(graph)?;
    match values.get(&root.id) {
        Some(NodeValue::City { mesh, instances }) => Ok(CityOutput {
            mesh: mesh.clone(),
            instances: instances.clone(),
        }),
        Some(_) => Err("CityRoot did not receive city output".into()),
        None => Err("CityRoot was not evaluated".into()),
    }
}

fn evaluate_all(graph: &Graph) -> Result<HashMap<NodeId, NodeValue>, String> {
    let mut values: HashMap<NodeId, NodeValue> = HashMap::new();
    let incoming: HashMap<NodeId, Vec<NodeId>> = graph
        .edges
        .iter()
        .fold(HashMap::new(), |mut map, edge| {
            map.entry(edge.to.clone())
                .or_default()
                .push(edge.from.clone());
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
            let seed = node
                .building_params
                .map(|p| p.seed)
                .unwrap_or(1);
            let instances = place_along_path(&path, seed);
            let merged = merge_instances(&mesh, &instances);
            Ok(NodeValue::City {
                mesh: merged,
                instances,
            })
        }
        NodeKind::FillGrid => {
            let mesh = input_mesh(inputs, cache)?;
            let grid = node.grid_input.clone().unwrap_or_default();
            let seed = node
                .building_params
                .map(|p| p.seed)
                .unwrap_or(1);
            let instances = fill_grid(&grid, seed);
            let merged = merge_instances(&mesh, &instances);
            Ok(NodeValue::City {
                mesh: merged,
                instances,
            })
        }
        NodeKind::MergeInstances => {
            let mut all_instances = Vec::new();
            let mut base_mesh: Option<Mesh> = None;
            for input_id in inputs {
                match cache.get(input_id) {
                    Some(NodeValue::City { mesh, instances }) => {
                        if base_mesh.is_none() {
                            base_mesh = Some(mesh.clone());
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
            let merged = merge_instances(&mesh, &all_instances);
            Ok(NodeValue::City {
                mesh: merged,
                instances: all_instances,
            })
        }
        NodeKind::CityRoot => {
            let input_id = inputs
                .first()
                .ok_or("CityRoot requires an input edge")?;
            match cache.get(input_id) {
                Some(NodeValue::City { mesh, instances }) => Ok(NodeValue::City {
                    mesh: mesh.clone(),
                    instances: instances.clone(),
                }),
                Some(NodeValue::Mesh(mesh)) => Ok(NodeValue::City {
                    mesh: mesh.clone(),
                    instances: vec![InstanceTransform::default()],
                }),
                _ => Err(format!("CityRoot input {} has unsupported type", input_id.0)),
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
    fn grid_graph_cooks() {
        let params_id = NodeId("params".into());
        let mesh_id = NodeId("mesh".into());
        let grid_id = NodeId("grid".into());
        let root_id = NodeId("root".into());
        let graph = Graph {
            name: "Grid Block".into(),
            nodes: vec![
                Node {
                    id: params_id.clone(),
                    kind: NodeKind::BuildingParams,
                    label: "Params".into(),
                    building_params: Some(BuildingParams::default()),
                    path_input: None,
                    grid_input: None,
                },
                Node {
                    id: mesh_id.clone(),
                    kind: NodeKind::BuildingMesh,
                    label: "Mesh".into(),
                    building_params: None,
                    path_input: None,
                    grid_input: None,
                },
                Node {
                    id: grid_id.clone(),
                    kind: NodeKind::FillGrid,
                    label: "Grid".into(),
                    building_params: None,
                    path_input: None,
                    grid_input: Some(GridInput::default()),
                },
                Node {
                    id: root_id.clone(),
                    kind: NodeKind::CityRoot,
                    label: "Root".into(),
                    building_params: None,
                    path_input: None,
                    grid_input: None,
                },
            ],
            edges: vec![
                Edge {
                    from: params_id.clone(),
                    to: mesh_id.clone(),
                },
                Edge {
                    from: mesh_id.clone(),
                    to: grid_id.clone(),
                },
                Edge {
                    from: grid_id.clone(),
                    to: root_id.clone(),
                },
            ],
        };
        let result = cook_graph(&graph).expect("cook");
        assert_eq!(result.instance_count, 6);
    }
}
