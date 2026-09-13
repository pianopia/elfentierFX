export type NodeKind =
  | "building_params"
  | "building_mesh"
  | "place_along_path"
  | "fill_grid"
  | "merge_instances"
  | "city_root"
  | "smoke_domain"
  | "smoke_source"
  | "smoke_collider"
  | "smoke_solver"
  | "smoke_root"
  | "liquid_domain"
  | "liquid_source"
  | "liquid_collider"
  | "liquid_solver"
  | "liquid_root";

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
  viscosity: number;
  pressure_iterations: number;
  ground_collision: boolean;
  max_particles_per_frame: number;
}

export type ColliderMode = "aabb" | "mesh_sdf";
export type ColliderMeshKind = "box" | "sphere" | "torus" | "ramp";

export interface ColliderInput {
  enabled: boolean;
  mode?: ColliderMode;
  bounds_min: Vec3;
  bounds_max: Vec3;
  bounce: number;
  kill_inside: boolean;
  mesh_kind?: ColliderMeshKind;
  mesh_resolution?: number;
  position?: Vec3;
  rotation_y?: number;
  scale?: Vec3;
}

export interface LiquidDomainInput {
  resolution: number;
  bounds_min: Vec3;
  bounds_max: Vec3;
  seed: number;
  initial_particles: number;
  particle_radius: number;
}

export interface LiquidSourceInput {
  position: Vec3;
  radius: number;
  emission_rate: number;
  velocity: Vec3;
  active_until_step: number;
}

export interface LiquidSolverInput {
  steps: number;
  frame_stride: number;
  gravity: number;
  flip_ratio: number;
  viscosity: number;
  pressure_iterations: number;
  wave_amplitude: number;
  wave_frequency: number;
  terrain_height: number;
  max_particles: number;
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
  smoke_collider?: ColliderInput | null;
  liquid_domain?: LiquidDomainInput | null;
  liquid_source?: LiquidSourceInput | null;
  liquid_solver?: LiquidSolverInput | null;
  liquid_collider?: ColliderInput | null;
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

export interface LiquidStats {
  resolution: [number, number, number];
  step_count: number;
  frame_count: number;
  particle_count: number;
  max_speed: number;
}

export interface ViewportLiquidFrame {
  positions: number[];
  radii: number[];
  opacities: number[];
  particle_count: number;
}

export interface ViewportLiquid {
  frames: ViewportLiquidFrame[];
  bounds_min: [number, number, number];
  bounds_max: [number, number, number];
  frame_count: number;
  fps: number;
  stats: LiquidStats;
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
  liquid_resolution?: [number, number, number] | null;
  liquid_steps?: number | null;
  liquid_frame_count?: number | null;
  liquid_particle_count?: number | null;
  liquid_max_speed?: number | null;
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
  native_preview_error?: string | null;
}

export interface ViewportColliderWireframe {
  bounds_min: [number, number, number];
  bounds_max: [number, number, number];
  lines: number[];
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
  liquid?: ViewportLiquid | null;
  colliders?: ViewportColliderWireframe[];
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

export interface LiquidExportResult {
  path: string;
  frame_count: number;
  byte_len: number;
  format: string;
}

export interface ExportPayloadEntry {
  format: string;
  path: string;
  frame_count?: number;
  byte_len?: number;
  vertex_count?: number;
  triangle_count?: number;
  bounds_min?: [number, number, number];
  bounds_max?: [number, number, number];
  resolution?: [number, number, number];
}

export interface ExportBundleResult {
  directory: string;
  manifest_path: string;
  payload_count: number;
  payloads: ExportPayloadEntry[];
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
  | { type: "increase_liquid"; emission_delta: number; wave_delta: number; step_delta: number }
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
    }
  | {
      type: "set_liquid_params";
      source_node_id: string;
      source: LiquidSourceInput;
      solver_node_id: string;
      solver: LiquidSolverInput;
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
  collider?: ColliderInput | null;
}

export interface SetLiquidParamsRequest {
  graph: Graph;
  domain?: LiquidDomainInput | null;
  source?: LiquidSourceInput | null;
  solver?: LiquidSolverInput | null;
  collider?: ColliderInput | null;
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
  smoke_collider: "Smoke Collider",
  smoke_solver: "Smoke Solver",
  smoke_root: "Smoke Root",
  liquid_domain: "Liquid Domain",
  liquid_source: "Liquid Source",
  liquid_collider: "Liquid Collider",
  liquid_solver: "Liquid Solver",
  liquid_root: "Liquid Surface",
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
  smoke_collider: "#f4a261",
  smoke_solver: "#b8f2e6",
  smoke_root: "#f5f5f5",
  liquid_domain: "#5ec8e8",
  liquid_source: "#4da6ff",
  liquid_collider: "#e9c46a",
  liquid_solver: "#7ad4f0",
  liquid_root: "#e8f8ff",
};
