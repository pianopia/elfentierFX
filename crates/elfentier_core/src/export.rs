//! glTF export and cook-session bundle packaging for engine/DCC handoff.

use crate::core_version;
use crate::graph::{
    cook_and_export, evaluate_liquid_volume, evaluate_smoke_volume, graph_mode, Graph, GraphMode,
};
use crate::liquid::export_particle_cache;
use crate::mesh::Mesh;
use crate::smoke::export_density_atlas;
use crate::volume_texture::{export_volume_texture, VOLUME_TEXTURE_FORMAT};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Manifest format identifier written to `manifest.json` in export bundles.
pub const EXPORT_MANIFEST_FORMAT: &str = "elfentier_export_manifest_v1";

/// Default playback frame rate for fluid payloads (matches viewport preview).
pub const EXPORT_FRAME_RATE: f32 = 12.0;

/// Linear unit convention for all spatial data in export bundles.
pub const EXPORT_UNITS: &str = "meters";

/// Up-axis convention for all spatial data in export bundles.
pub const EXPORT_UP_AXIS: &str = "Y";

/// Result of an export operation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExportResult {
    pub path: String,
    pub vertex_count: u32,
    pub triangle_count: u32,
    pub byte_len: usize,
}

/// Writes a minimal binary glTF (.glb) of a merged mesh to `path`.
pub fn export_glb(mesh: &Mesh, path: &str) -> std::io::Result<ExportResult> {
    let positions: Vec<f32> = mesh
        .positions
        .iter()
        .flat_map(|p| [p.x, p.y, p.z])
        .collect();
    let indices = &mesh.indices;

    let pos_bytes = f32_slice_to_bytes(&positions);
    let idx_bytes = u32_slice_to_bytes(indices);

    let pos_len = pos_bytes.len();
    let idx_len = idx_bytes.len();
    let buffer_len = pos_len + idx_len;

    let min = bounding_min(&mesh.positions);
    let max = bounding_max(&mesh.positions);

    let json = format!(
        r#"{{
  "asset": {{"version": "2.0", "generator": "elfentierFX"}},
  "scene": 0,
  "scenes": [{{"nodes": [0]}}],
  "nodes": [{{"mesh": 0, "name": "{}"}}],
  "meshes": [{{"primitives": [{{"attributes": {{"POSITION": 0}}, "indices": 1, "mode": 4}}]}}],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": {}, "type": "VEC3", "min": [{}, {}, {}], "max": [{}, {}, {}]}},
    {{"bufferView": 1, "componentType": 5125, "count": {}, "type": "SCALAR"}}
  ],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": {}, "target": 34962}},
    {{"buffer": 0, "byteOffset": {}, "byteLength": {}, "target": 34963}}
  ],
  "buffers": [{{"byteLength": {}}}]
}}"#,
        escape_json(&mesh.name),
        mesh.positions.len(),
        min.x,
        min.y,
        min.z,
        max.x,
        max.y,
        max.z,
        indices.len(),
        pos_len,
        pos_len,
        idx_len,
        buffer_len,
    );

    let json_bytes = json.as_bytes();
    let json_pad = (4 - (json_bytes.len() % 4)) % 4;
    let bin_pad = (4 - (buffer_len % 4)) % 4;
    let total_len = 12 + 8 + json_bytes.len() + json_pad + 8 + buffer_len + bin_pad;

    let mut file = std::fs::File::create(path)?;
    file.write_all(&12u32.to_le_bytes())?;
    file.write_all(&(total_len as u32).to_le_bytes())?;
    file.write_all(b"glTF")?;
    file.write_all(&(json_bytes.len() as u32 + json_pad as u32).to_le_bytes())?;
    file.write_all(b"JSON")?;
    file.write_all(json_bytes)?;
    for _ in 0..json_pad {
        file.write_all(b" ")?;
    }
    file.write_all(&((buffer_len + bin_pad) as u32).to_le_bytes())?;
    file.write_all(b"BIN\x00")?;
    file.write_all(&pos_bytes)?;
    file.write_all(&idx_bytes)?;
    for _ in 0..bin_pad {
        file.write_all(&[0])?;
    }

    Ok(ExportResult {
        path: path.to_string(),
        vertex_count: mesh.vertex_count(),
        triangle_count: mesh.triangle_count(),
        byte_len: total_len,
    })
}

fn f32_slice_to_bytes(data: &[f32]) -> Vec<u8> {
    data.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn u32_slice_to_bytes(data: &[u32]) -> Vec<u8> {
    data.iter().flat_map(|u| u.to_le_bytes()).collect()
}

fn bounding_min(positions: &[crate::mesh::Vec3]) -> crate::mesh::Vec3 {
    let mut min = crate::mesh::Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    for p in positions {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        min.z = min.z.min(p.z);
    }
    min
}

fn bounding_max(positions: &[crate::mesh::Vec3]) -> crate::mesh::Vec3 {
    let mut max = crate::mesh::Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in positions {
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
        max.z = max.z.max(p.z);
    }
    max
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// One payload entry referenced by an export manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportPayloadEntry {
    pub format: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_len: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertex_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triangle_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds_min: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds_max: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<[u32; 3]>,
}

/// Unified manifest describing a cooked export bundle directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportManifest {
    pub format: String,
    pub version: u32,
    pub core_version: String,
    pub created_at: String,
    pub graph_name: String,
    pub graph_mode: String,
    pub units: String,
    pub up_axis: String,
    pub frame_rate: f32,
    pub payloads: Vec<ExportPayloadEntry>,
}

/// Result of writing a cook export bundle directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportBundleResult {
    pub directory: String,
    pub manifest_path: String,
    pub payload_count: usize,
    pub payloads: Vec<ExportPayloadEntry>,
}

/// Creates a bundle directory under the system temp folder when `path` is None.
pub fn default_bundle_directory(graph_name: &str) -> std::io::Result<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let slug = sanitize_dir_name(graph_name);
    let dir = std::env::temp_dir().join(format!("elfentier_export_{slug}_{stamp}"));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Writes a cook export bundle (manifest + payloads) for the given graph.
pub fn export_cook_bundle(graph: &Graph, path: Option<&str>) -> Result<ExportBundleResult, String> {
    let dir = match path {
        Some(p) => {
            let dir = PathBuf::from(p);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            dir
        }
        None => default_bundle_directory(&graph.name).map_err(|e| e.to_string())?,
    };

    let graph_path = dir.join("graph.json");
    let graph_json = serde_json::to_string_pretty(graph).map_err(|e| e.to_string())?;
    std::fs::write(&graph_path, &graph_json).map_err(|e| e.to_string())?;

    let mut payloads = vec![ExportPayloadEntry {
        format: "elfentier_graph_v1".into(),
        path: "graph.json".into(),
        frame_count: None,
        byte_len: Some(graph_json.len()),
        vertex_count: None,
        triangle_count: None,
        bounds_min: None,
        bounds_max: None,
        resolution: None,
    }];

    match graph_mode(graph) {
        GraphMode::City => {
            let mesh_path = dir.join("city_mesh.glb");
            let mesh_path_str = mesh_path.to_string_lossy().to_string();
            let mesh_result = cook_and_export(graph, &mesh_path_str)?;
            payloads.push(ExportPayloadEntry {
                format: "gltf_glb".into(),
                path: "city_mesh.glb".into(),
                frame_count: None,
                byte_len: Some(mesh_result.byte_len),
                vertex_count: Some(mesh_result.vertex_count),
                triangle_count: Some(mesh_result.triangle_count),
                bounds_min: None,
                bounds_max: None,
                resolution: None,
            });
        }
        GraphMode::Smoke => {
            let volume = evaluate_smoke_volume(graph)?;
            let smoke_path = dir.join("smoke_density.raw");
            let smoke_path_str = smoke_path.to_string_lossy().to_string();
            let smoke_result =
                export_density_atlas(&volume, &smoke_path_str).map_err(|e| e.to_string())?;
            payloads.push(ExportPayloadEntry {
                format: smoke_result.format.clone(),
                path: "smoke_density.raw".into(),
                frame_count: Some(smoke_result.frame_count),
                byte_len: Some(smoke_result.byte_len),
                vertex_count: None,
                triangle_count: None,
                bounds_min: Some(vec3_to_array(volume.bounds_min)),
                bounds_max: Some(vec3_to_array(volume.bounds_max)),
                resolution: Some(volume.stats.resolution),
            });

            let volume_path = dir.join("volume_texture.evol");
            let volume_path_str = volume_path.to_string_lossy().to_string();
            let volume_result =
                export_volume_texture(&volume, &volume_path_str).map_err(|e| e.to_string())?;
            payloads.push(ExportPayloadEntry {
                format: VOLUME_TEXTURE_FORMAT.into(),
                path: "volume_texture.evol".into(),
                frame_count: Some(volume_result.frame_count),
                byte_len: Some(volume_result.byte_len),
                vertex_count: None,
                triangle_count: None,
                bounds_min: Some(vec3_to_array(volume.bounds_min)),
                bounds_max: Some(vec3_to_array(volume.bounds_max)),
                resolution: Some(volume_result.resolution),
            });
        }
        GraphMode::Liquid => {
            let volume = evaluate_liquid_volume(graph)?;
            let liquid_path = dir.join("liquid_cache.raw");
            let liquid_path_str = liquid_path.to_string_lossy().to_string();
            let liquid_result =
                export_particle_cache(&volume, &liquid_path_str).map_err(|e| e.to_string())?;
            payloads.push(ExportPayloadEntry {
                format: liquid_result.format.clone(),
                path: "liquid_cache.raw".into(),
                frame_count: Some(liquid_result.frame_count),
                byte_len: Some(liquid_result.byte_len),
                vertex_count: None,
                triangle_count: None,
                bounds_min: Some(vec3_to_array(volume.bounds_min)),
                bounds_max: Some(vec3_to_array(volume.bounds_max)),
                resolution: Some(volume.stats.resolution),
            });
        }
    }

    let manifest = ExportManifest {
        format: EXPORT_MANIFEST_FORMAT.into(),
        version: 1,
        core_version: core_version().into(),
        created_at: iso8601_now(),
        graph_name: graph.name.clone(),
        graph_mode: graph_mode_label(graph_mode(graph)).into(),
        units: EXPORT_UNITS.into(),
        up_axis: EXPORT_UP_AXIS.into(),
        frame_rate: EXPORT_FRAME_RATE,
        payloads: payloads.clone(),
    };

    let manifest_path = dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(&manifest_path, &manifest_json).map_err(|e| e.to_string())?;

    Ok(ExportBundleResult {
        directory: dir.to_string_lossy().to_string(),
        manifest_path: manifest_path.to_string_lossy().to_string(),
        payload_count: payloads.len(),
        payloads,
    })
}

fn vec3_to_array(v: crate::mesh::Vec3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn graph_mode_label(mode: GraphMode) -> &'static str {
    match mode {
        GraphMode::City => "city",
        GraphMode::Smoke => "smoke",
        GraphMode::Liquid => "liquid",
    }
}

fn sanitize_dir_name(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('_');
    if trimmed.is_empty() {
        "cook".into()
    } else {
        trimmed.chars().take(48).collect()
    }
}

fn iso8601_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

/// Reads and parses a bundle manifest from disk (for tests and integrations).
pub fn read_export_manifest(path: &Path) -> Result<ExportManifest, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;
    use crate::mesh::create_unit_box_mesh;

    #[test]
    fn exports_glb_file() {
        let mesh = create_unit_box_mesh();
        let dir = std::env::temp_dir();
        let path = dir.join("elfentier_test_export.glb");
        let path_str = path.to_string_lossy().to_string();
        let result = export_glb(&mesh, &path_str).expect("export");
        assert!(std::path::Path::new(&path_str).exists());
        assert_eq!(result.vertex_count, 8);
        assert!(result.byte_len > 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn city_bundle_writes_manifest_and_glb() {
        let graph = Graph::shop_street_preset();
        let dir = std::env::temp_dir().join("elfentier_test_city_bundle");
        let _ = std::fs::remove_dir_all(&dir);
        let result = export_cook_bundle(&graph, Some(dir.to_str().unwrap())).expect("bundle");
        assert_eq!(result.payload_count, 2);
        assert!(Path::new(&result.manifest_path).exists());
        let manifest = read_export_manifest(Path::new(&result.manifest_path)).expect("manifest");
        assert_eq!(manifest.format, EXPORT_MANIFEST_FORMAT);
        assert_eq!(manifest.units, EXPORT_UNITS);
        assert_eq!(manifest.up_axis, EXPORT_UP_AXIS);
        assert_eq!(manifest.frame_rate, EXPORT_FRAME_RATE);
        assert_eq!(manifest.graph_mode, "city");
        assert!(dir.join("city_mesh.glb").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn smoke_bundle_writes_smoke_atlas_and_volume_texture_payloads() {
        let graph = Graph::smoke_puff_preset();
        let dir = std::env::temp_dir().join("elfentier_test_smoke_bundle");
        let _ = std::fs::remove_dir_all(&dir);
        let result = export_cook_bundle(&graph, Some(dir.to_str().unwrap())).expect("bundle");
        let manifest = read_export_manifest(Path::new(&result.manifest_path)).expect("manifest");
        assert_eq!(manifest.graph_mode, "smoke");
        assert!(manifest
            .payloads
            .iter()
            .any(|p| p.format == "elfentier_smoke_atlas_v1"));
        assert!(manifest
            .payloads
            .iter()
            .any(|p| p.format == VOLUME_TEXTURE_FORMAT));
        assert!(dir.join("smoke_density.raw").exists());
        assert!(dir.join("volume_texture.evol").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn liquid_bundle_writes_liquid_cache_payload() {
        let graph = Graph::ocean_patch_preset();
        let dir = std::env::temp_dir().join("elfentier_test_liquid_bundle");
        let _ = std::fs::remove_dir_all(&dir);
        let result = export_cook_bundle(&graph, Some(dir.to_str().unwrap())).expect("bundle");
        let manifest = read_export_manifest(Path::new(&result.manifest_path)).expect("manifest");
        assert_eq!(manifest.graph_mode, "liquid");
        assert!(manifest
            .payloads
            .iter()
            .any(|p| p.format == "elfentier_liquid_cache_v1"));
        assert!(dir.join("liquid_cache.raw").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
