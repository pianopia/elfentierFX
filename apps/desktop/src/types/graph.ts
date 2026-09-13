export type NodeKind =
  | "building_params"
  | "building_mesh"
  | "place_along_path"
  | "fill_grid"
  | "merge_instances"
  | "city_root"
  | "smoke_domain"
  | "smoke_source"
  | "smoke_solver"
  | "smoke_root";

export type GraphOutputKind = "city" | "smoke";

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

export interface SmokeDomainParams {
  nx: number;
  ny: number;
  nz: number;
  bounds_min: [number, number, number];
  bounds_max: [number, number, number];
}

export interface SmokeSourceParams {
  position: [number, number, number];
  radius: number;
  emit_rate: number;
  emit_velocity: [number, number, number];
}

export interface SmokeSolverParams {
  steps: number;
  dt: number;
  diffusion: number;
  buoyancy: number;
  pressure_iterations: number;
  max_density: number;
}

export interface GraphNode {
  id: string;
  kind: NodeKind;
  label: string;
  building_params?: BuildingParams | null;
  path_input?: PathInput | null;
  grid_input?: GridInput | null;
  smoke_domain?: SmokeDomainParams | null;
  smoke_source?: SmokeSourceParams | null;
  smoke_solver?: SmokeSolverParams | null;
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
  output_kind: GraphOutputKind | string;
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

export interface ViewportSmoke {
  nx: number;
  ny: number;
  nz: number;
  bounds_min: [number, number, number];
  bounds_max: [number, number, number];
  density: number[];
  max_density: number;
  graph_name: string;
}

export interface ViewportCook {
  output_kind: GraphOutputKind;
  mesh: ViewportMesh | null;
  smoke: ViewportSmoke | null;
}

export interface SmokePreviewImage {
  width: number;
  height: number;
  rgba: number[];
}

export interface CookWithViewportResult {
  stats: CookResult;
  viewport: ViewportCook;
  smoke_preview: SmokePreviewImage | null;
}

/** @deprecated Use CookWithViewportResult */
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

export interface SmokeVolumeExport {
  path: string;
  format: string;
  nx: number;
  ny: number;
  nz: number;
  frame_count: number;
  byte_len: number;
  notes: string;
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
  smoke_domain: "Smoke Domain",
  smoke_source: "Smoke Source",
  smoke_solver: "Smoke Solver",
  smoke_root: "Smoke Root",
};

export const NODE_KIND_COLORS: Record<NodeKind, string> = {
  building_params: "#6ea8ff",
  building_mesh: "#7ee8a2",
  place_along_path: "#f0b35a",
  fill_grid: "#c98bff",
  merge_instances: "#ff8fa3",
  city_root: "#e8e8e8",
  smoke_domain: "#9ad4ff",
  smoke_source: "#ffd28a",
  smoke_solver: "#b8a0ff",
  smoke_root: "#f5f5f5",
};
