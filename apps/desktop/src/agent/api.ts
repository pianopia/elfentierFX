import { invoke } from "@tauri-apps/api/core";
import type {
  ApplyPromptResult,
  BuildingParams,
  CookResult,
  CookWithMeshResult,
  ExportResult,
  Graph,
  PresetInfo,
  SetSmokeParamsRequest,
  SmokeExportResult,
  ViewportMesh,
} from "../types/graph";

/** Agent-oriented command surface — mirrors Tauri invoke handlers. */
export const agentApi = {
  getCoreVersion: () => invoke<string>("get_core_version"),

  getGraph: (graph: Graph) => invoke<Graph>("get_graph", { graph }),

  listPresets: () => invoke<PresetInfo[]>("list_presets_command"),

  loadPreset: (presetId: string) => invoke<Graph>("load_preset", { presetId }),

  getShopStreetPreset: () => invoke<Graph>("get_shop_street_preset"),

  getSmokePuffPreset: () => invoke<Graph>("get_smoke_puff_preset"),

  setParams: (graph: Graph, params: BuildingParams, nodeId?: string) =>
    invoke<Graph>("set_params_command", {
      request: { graph, node_id: nodeId ?? null, params },
    }),

  setSmokeParams: (request: SetSmokeParamsRequest) =>
    invoke<Graph>("set_smoke_params_command", { request }),

  cook: (graph: Graph) => invoke<CookWithMeshResult>("cook", { graph }),

  cookStats: (graph: Graph) => invoke<CookResult>("cook_city_graph", { graph }),

  exportGltf: (graph: Graph, path: string) =>
    invoke<ExportResult>("export_gltf", { graph, path }),

  exportSmokeDensity: (graph: Graph, path: string) =>
    invoke<SmokeExportResult>("export_smoke_density_command", {
      request: { graph, path },
    }),

  applyPrompt: (graph: Graph, prompt: string) =>
    invoke<ApplyPromptResult>("apply_prompt_command", {
      request: { graph, prompt },
    }),

  explainGraph: (graph: Graph) => invoke<string>("explain_graph_command", { graph }),
};

export type { CookWithMeshResult, ViewportMesh };
