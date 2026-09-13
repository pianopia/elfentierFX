use elfentier_core::{create_box_mesh, core_version, MeshStats};

#[tauri::command]
fn get_core_version() -> String {
    core_version().to_string()
}

#[tauri::command]
fn create_box_mesh_command() -> MeshStats {
    create_box_mesh()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_core_version,
            create_box_mesh_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running elfentierFX");
}
