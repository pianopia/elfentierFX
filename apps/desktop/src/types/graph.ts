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

export interface SmokeDomainInput {
  resolution: number;
  bounds_min: Vec3;
  bounds_max: Vec3;
  seed: number;
}

export interface SmokeSourceInput {
  position: Vec3;
  radius: number;
  emission_rate: number;
  temperature: number;
  upward_velocity: number;
}

export interface SmokeSolverInput {
  steps: number;
  frame_stride: number;
  dissipation: number;
  buoyancy: number;
  diffusion: number;
  pressure_iterations: number;
  ground_collision: boolean;
  max_particles_per_frame: number;
}

export interface GraphNode {
  id: string;
  kind: NodeKind;
  label: string;
  building_params?: BuildingParams | null;
  path_input?: PathInput | null;
  grid_input?: GridInput | null;
  smoke_domain?: SmokeDomainInput | null;
  smoke_source?: SmokeSourceInput | null;
  smoke_solver?: SmokeSolverInput | null;
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

export interface SmokeStats {
  resolution: [number, number, number];
  max_density: number;
  step_count: number;
  frame_count: number;
  particle_count: number;
  total_density: number;
}

export interface ViewportSmokeFrame {
  positions: number[];
  sizes: number[];
  opacities: number[];
  particle_count: number;
}

export interface ViewportSmoke {
  frames: ViewportSmokeFrame[];
  bounds_min: [number, number, number];
  bounds_max: [number, number, number];
  frame_count: number;
  fps: number;
  stats: SmokeStats;
}

export interface CookResult {
  vertex_count: number;
  index_count: number;
  triangle_count: number;
  instance_count: number;
  graph_name: string;
  smoke_resolution?: [number, number, number] | null;
  smoke_max_density?: number | null;
  smoke_steps?: number | null;
  smoke_frame_count?: number | null;
  smoke_particle_count?: number | null;
  native_viewport?: boolean | null;
}

export interface NativeViewportCamera {
  eye: [number, number, number];
  target: [number, number, number];
  up: [number, number, number];
  fov_y_deg: number;
}

export interface NativePreviewImage {
  width: number;
  height: number;
  rgba: number[];
  backend: string;
}

export interface CookWithMeshResult {
  stats: CookResult;
  mesh: ViewportMesh;
  native_preview?: NativePreviewImage | null;
  native_camera?: NativeViewportCamera | null;
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
  smoke?: ViewportSmoke | null;
}

export interface ExportResult {
  path: string;
  vertex_count: number;
  triangle_count: number;
  byte_len: number;
}

export interface SmokeExportResult {
  path: string;
  frame_count: number;
  byte_len: number;
  format: string;
}

export type PlacementMode = "along_path" | "grid";

export type PromptIntent =
  | { type: "load_preset"; preset_id: string }
  | { type: "set_floors"; floors: number }
  | { type: "set_seed"; seed: number }
  | { type: "adjust_windows"; delta: number }
  | { type: "switch_placement"; mode: PlacementMode }
  | { type: "set_graph_name"; name: string }
  | { type: "increase_smoke"; emission_delta: number; step_delta: number }
  | { type: "unknown"; raw: string };

export type GraphEdit =
  | { type: "load_preset"; preset_id: string }
  | { type: "set_building_params"; node_id: string; params: BuildingParams }
  | { type: "switch_placement"; mode: PlacementMode }
  | { type: "set_graph_name"; name: string }
  | {
      type: "set_smoke_params";
      source_node_id: string;
      source: SmokeSourceInput;
      solver_node_id: string;
      solver: SmokeSolverInput;
    };

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

export interface SetSmokeParamsRequest {
  graph: Graph;
  domain?: SmokeDomainInput | null;
  source?: SmokeSourceInput | null;
  solver?: SmokeSolverInput | null;
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
  smoke_source: "#ffd166",
  smoke_solver: "#b8f2e6",
  smoke_root: "#f5f5f5",
};
