mod wgpu_viewport;

use elfentier_core::{
    agent::{
        export_cook_bundle_command, export_liquid_cache, export_smoke_density, export_smoke_vdb,
        get_preset,
        list_presets, set_liquid_params, set_params, set_smoke_params, PresetInfo,
        SetLiquidParamsRequest, SetSmokeParamsRequest,
    },
    building::BuildingParams,
    core_version, create_box_mesh,
    explain::explain_graph,
    export::ExportBundleResult,
    graph::{cook_and_export, cook_graph, Graph},
    prompt::{apply_prompt, ApplyPromptResult},
    liquid::LiquidExportResult,
    openvdb_io::OpenVdbExportResult,
    smoke::SmokeExportResult,
    viewport::{cook_viewport_mesh, ViewportMesh},
    MeshStats,
};
use serde::{Deserialize, Serialize};
use wgpu_viewport::{
    default_camera_for_mesh, render_native_viewport, NativePreviewImage, NativeViewportCamera,
};

#[derive(Debug, Serialize, Deserialize)]
struct CookResult {
    vertex_count: u32,
    index_count: u32,
    triangle_count: u32,
    instance_count: u32,
    graph_name: String,
    smoke_resolution: Option<[u32; 3]>,
    smoke_max_density: Option<f32>,
    smoke_steps: Option<u32>,
    smoke_frame_count: Option<u32>,
    smoke_particle_count: Option<u32>,
    liquid_resolution: Option<[u32; 3]>,
    liquid_steps: Option<u32>,
    liquid_frame_count: Option<u32>,
    liquid_particle_count: Option<u32>,
    liquid_max_speed: Option<f32>,
    native_viewport: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct CookWithMeshResult {
    stats: CookResult,
    mesh: ViewportMesh,
    native_preview: Option<NativePreviewImage>,
    native_camera: Option<NativeViewportCamera>,
    native_preview_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RenderNativeRequest {
    mesh: ViewportMesh,
    smoke_frame: u32,
    width: u32,
    height: u32,
    camera: NativeViewportCamera,
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

#[derive(Debug, Serialize, Deserialize)]
struct ExportSmokeRequest {
    graph: Graph,
    path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExportLiquidRequest {
    graph: Graph,
    path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExportBundleRequest {
    graph: Graph,
    path: Option<String>,
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
fn get_smoke_puff_preset() -> Graph {
    Graph::smoke_puff_preset()
}

#[tauri::command]
fn get_ocean_patch_preset() -> Graph {
    Graph::ocean_patch_preset()
}

#[tauri::command]
fn get_waterfall_preset() -> Graph {
    Graph::waterfall_preset()
}

#[tauri::command]
fn get_flood_basin_preset() -> Graph {
    Graph::flood_basin_preset()
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
fn set_smoke_params_command(request: SetSmokeParamsRequest) -> Graph {
    set_smoke_params(&request)
}

#[tauri::command]
fn set_liquid_params_command(request: SetLiquidParamsRequest) -> Graph {
    set_liquid_params(&request)
}

#[tauri::command]
fn cook_city_graph(graph: Graph) -> Result<CookResult, String> {
    let result = cook_graph(&graph)?;
    Ok(map_cook_result(result, false))
}

#[tauri::command]
fn cook(graph: Graph) -> Result<CookWithMeshResult, String> {
    let stats = cook_graph(&graph)?;
    let mesh = cook_viewport_mesh(&graph)?;
    let camera = default_camera_for_mesh(&mesh);
    let (native_preview, native_preview_error) =
        match render_native_viewport(&mesh, 0, 960, 720, &camera) {
            Ok(preview) => (Some(preview), None),
            Err(err) => {
                eprintln!("native viewport preview failed: {err}");
                (None, Some(err))
            }
        };
    let native_viewport = native_preview.is_some();
    Ok(CookWithMeshResult {
        stats: map_cook_result(stats, native_viewport),
        mesh,
        native_preview,
        native_camera: Some(camera),
        native_preview_error,
    })
}

#[tauri::command]
fn render_native_viewport_command(request: RenderNativeRequest) -> Result<NativePreviewImage, String> {
    render_native_viewport(
        &request.mesh,
        request.smoke_frame,
        request.width,
        request.height,
        &request.camera,
    )
}

fn map_cook_result(result: elfentier_core::graph::CookResult, native_viewport: bool) -> CookResult {
    CookResult {
        vertex_count: result.vertex_count,
        index_count: result.index_count,
        triangle_count: result.triangle_count,
        instance_count: result.instance_count,
        graph_name: result.graph_name,
        smoke_resolution: result.smoke_resolution,
        smoke_max_density: result.smoke_max_density,
        smoke_steps: result.smoke_steps,
        smoke_frame_count: result.smoke_frame_count,
        smoke_particle_count: result.smoke_particle_count,
        liquid_resolution: result.liquid_resolution,
        liquid_steps: result.liquid_steps,
        liquid_frame_count: result.liquid_frame_count,
        liquid_particle_count: result.liquid_particle_count,
        liquid_max_speed: result.liquid_max_speed,
        native_viewport,
    }
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
fn export_smoke_density_command(request: ExportSmokeRequest) -> Result<SmokeExportResult, String> {
    export_smoke_density(&request.graph, &request.path)
}

#[tauri::command]
fn export_smoke_vdb_command(request: ExportSmokeRequest) -> Result<OpenVdbExportResult, String> {
    export_smoke_vdb(&request.graph, &request.path)
}

#[tauri::command]
fn export_liquid_cache_command(request: ExportLiquidRequest) -> Result<LiquidExportResult, String> {
    export_liquid_cache(&request.graph, &request.path)
}

#[tauri::command]
fn export_cook_bundle_command_handler(
    request: ExportBundleRequest,
) -> Result<ExportBundleResult, String> {
    export_cook_bundle_command(&request.graph, request.path.as_deref())
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
            get_smoke_puff_preset,
            get_ocean_patch_preset,
            get_waterfall_preset,
            get_flood_basin_preset,
            get_graph,
            list_presets_command,
            set_params_command,
            set_smoke_params_command,
            set_liquid_params_command,
            cook_city_graph,
            cook,
            render_native_viewport_command,
            export_city_graph,
            export_gltf,
            export_smoke_density_command,
            export_smoke_vdb_command,
            export_liquid_cache_command,
            export_cook_bundle_command_handler,
            apply_prompt_command,
            explain_graph_command,
            load_preset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running elfentierFX");
}
