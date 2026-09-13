mod wgpu_viewport;

use elfentier_core::{
    agent::{get_preset, list_presets, set_params, PresetInfo},
    building::BuildingParams,
    core_version, create_box_mesh,
    explain::explain_graph,
    export::{export_smoke_volume_raw, SmokeVolumeExport},
    graph::{cook_and_export, cook_graph, graph_output_kind, Graph, GraphOutputKind},
    prompt::{apply_prompt, ApplyPromptResult},
    viewport::{cook_viewport, ViewportCook, SmokePreviewImage},
    MeshStats,
};
use serde::{Deserialize, Serialize};
use wgpu_viewport::render_smoke_preview;

#[derive(Debug, Serialize, Deserialize)]
struct CookResult {
    vertex_count: u32,
    index_count: u32,
    triangle_count: u32,
    instance_count: u32,
    graph_name: String,
    output_kind: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CookWithViewportResult {
    stats: CookResult,
    viewport: ViewportCook,
    smoke_preview: Option<SmokePreviewImage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExportResult {
    path: String,
    vertex_count: u32,
    triangle_count: u32,
    byte_len: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct SetParamsRequest {
    graph: Graph,
    node_id: Option<String>,
    params: BuildingParams,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApplyPromptRequest {
    graph: Graph,
    prompt: String,
}

#[tauri::command]
fn get_core_version() -> String {
    core_version().to_string()
}

#[tauri::command]
fn create_box_mesh_command() -> MeshStats {
    create_box_mesh()
}

#[tauri::command]
fn get_shop_street_preset() -> Graph {
    Graph::shop_street_preset()
}

#[tauri::command]
fn get_smoke_plume_preset() -> Graph {
    Graph::smoke_plume_preset()
}

#[tauri::command]
fn get_graph(graph: Graph) -> Graph {
    graph
}

#[tauri::command]
fn list_presets_command() -> Vec<PresetInfo> {
    list_presets()
}

#[tauri::command]
fn set_params_command(request: SetParamsRequest) -> Graph {
    set_params(&request.graph, request.node_id.as_deref(), request.params)
}

#[tauri::command]
fn cook_city_graph(graph: Graph) -> Result<CookResult, String> {
    let result = cook_graph(&graph)?;
    let kind = graph_output_kind(&graph);
    Ok(CookResult {
        vertex_count: result.vertex_count,
        index_count: result.index_count,
        triangle_count: result.triangle_count,
        instance_count: result.instance_count,
        graph_name: result.graph_name,
        output_kind: match kind {
            GraphOutputKind::City => "city".into(),
            GraphOutputKind::Smoke => "smoke".into(),
        },
    })
}

#[tauri::command]
fn cook(graph: Graph) -> Result<CookWithViewportResult, String> {
    let stats = cook_graph(&graph)?;
    let viewport = cook_viewport(&graph)?;
    let smoke_preview = if let Some(smoke) = viewport.smoke.as_ref() {
        Some(render_smoke_preview(smoke, 640, 480)?)
    } else {
        None
    };
    Ok(CookWithViewportResult {
        stats: CookResult {
            vertex_count: stats.vertex_count,
            index_count: stats.index_count,
            triangle_count: stats.triangle_count,
            instance_count: stats.instance_count,
            graph_name: stats.graph_name,
            output_kind: match viewport.output_kind {
                GraphOutputKind::City => "city".into(),
                GraphOutputKind::Smoke => "smoke".into(),
            },
        },
        viewport,
        smoke_preview,
    })
}

#[tauri::command]
fn export_city_graph(graph: Graph, path: String) -> Result<ExportResult, String> {
    let result = cook_and_export(&graph, &path)?;
    Ok(ExportResult {
        path: result.path,
        vertex_count: result.vertex_count,
        triangle_count: result.triangle_count,
        byte_len: result.byte_len,
    })
}

#[tauri::command]
fn export_gltf(graph: Graph, path: String) -> Result<ExportResult, String> {
    export_city_graph(graph, path)
}

#[tauri::command]
fn export_smoke_volume(graph: Graph, path: String) -> Result<SmokeVolumeExport, String> {
    let grid = elfentier_core::graph::evaluate_smoke(&graph)?;
    export_smoke_volume_raw(&grid, &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn apply_prompt_command(request: ApplyPromptRequest) -> Result<ApplyPromptResult, String> {
    Ok(apply_prompt(&request.graph, &request.prompt))
}

#[tauri::command]
fn explain_graph_command(graph: Graph) -> String {
    explain_graph(&graph)
}

#[tauri::command]
fn load_preset(preset_id: String) -> Result<Graph, String> {
    get_preset(&preset_id).ok_or_else(|| format!("unknown preset: {}", preset_id))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_core_version,
            create_box_mesh_command,
            get_shop_street_preset,
            get_smoke_plume_preset,
            get_graph,
            list_presets_command,
            set_params_command,
            cook_city_graph,
            cook,
            export_city_graph,
            export_gltf,
            export_smoke_volume,
            apply_prompt_command,
            explain_graph_command,
            load_preset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running elfentierFX");
}
