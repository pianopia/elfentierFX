use elfentier_core::{
    core_version, create_box_mesh,
    graph::{cook_and_export, cook_graph, Graph},
    MeshStats,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct CookResult {
    vertex_count: u32,
    index_count: u32,
    triangle_count: u32,
    instance_count: u32,
    graph_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExportResult {
    path: String,
    vertex_count: u32,
    triangle_count: u32,
    byte_len: usize,
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
fn cook_city_graph(graph: Graph) -> Result<CookResult, String> {
    let result = cook_graph(&graph)?;
    Ok(CookResult {
        vertex_count: result.vertex_count,
        index_count: result.index_count,
        triangle_count: result.triangle_count,
        instance_count: result.instance_count,
        graph_name: result.graph_name,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_core_version,
            create_box_mesh_command,
            get_shop_street_preset,
            cook_city_graph,
            export_city_graph
        ])
        .run(tauri::generate_context!())
        .expect("error while running elfentierFX");
}
