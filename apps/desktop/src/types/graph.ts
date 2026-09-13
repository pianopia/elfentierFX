export type NodeKind =
  | "building_params"
  | "building_mesh"
  | "place_along_path"
  | "fill_grid"
  | "merge_instances"
  | "city_root";

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

export interface BuildingParams {
  floors: number;
  width: number;
  depth: number;
  window_density: number;
  seed: number;
  floor_height: number;
}

export interface PathInput {
  start: Vec3;
  end: Vec3;
  spacing: number;
  offset_from_path: number;
}

export interface GridInput {
  origin: Vec3;
  cols: number;
  rows: number;
  lot_width: number;
  lot_depth: number;
  spacing: number;
}

export interface GraphNode {
  id: string;
  kind: NodeKind;
  label: string;
  building_params?: BuildingParams | null;
  path_input?: PathInput | null;
  grid_input?: GridInput | null;
}

export interface GraphEdge {
  from: string;
  to: string;
}

export interface Graph {
  name: string;
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface CookResult {
  vertex_count: number;
  index_count: number;
  triangle_count: number;
  instance_count: number;
  graph_name: string;
}

export interface ViewportMesh {
  positions: number[];
  indices: number[];
  instance_matrices: number[];
  vertex_count: number;
  index_count: number;
  triangle_count: number;
  instance_count: number;
  graph_name: string;
}

export interface CookWithMeshResult {
  stats: CookResult;
  mesh: ViewportMesh;
}

export interface ExportResult {
  path: string;
  vertex_count: number;
  triangle_count: number;
  byte_len: number;
}

export type PlacementMode = "along_path" | "grid";

export type PromptIntent =
  | { type: "load_preset"; preset_id: string }
  | { type: "set_floors"; floors: number }
  | { type: "set_seed"; seed: number }
  | { type: "adjust_windows"; delta: number }
  | { type: "switch_placement"; mode: PlacementMode }
  | { type: "set_graph_name"; name: string }
  | { type: "unknown"; raw: string };

export type GraphEdit =
  | { type: "load_preset"; preset_id: string }
  | { type: "set_building_params"; node_id: string; params: BuildingParams }
  | { type: "switch_placement"; mode: PlacementMode }
  | { type: "set_graph_name"; name: string };

export interface ApplyPromptResult {
  graph: Graph;
  intents: PromptIntent[];
  edits: GraphEdit[];
  summary: string;
}

export interface PresetInfo {
  id: string;
  name: string;
  description: string;
}

export const NODE_KIND_LABELS: Record<NodeKind, string> = {
  building_params: "Building Params",
  building_mesh: "Building Mesh",
  place_along_path: "Place Along Path",
  fill_grid: "Fill Grid",
  merge_instances: "Merge Instances",
  city_root: "City Root",
};

export const NODE_KIND_COLORS: Record<NodeKind, string> = {
  building_params: "#6ea8ff",
  building_mesh: "#7ee8a2",
  place_along_path: "#f0b35a",
  fill_grid: "#c98bff",
  merge_instances: "#ff8fa3",
  city_root: "#e8e8e8",
};
