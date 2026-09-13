//! Minimal glTF export for cooked city geometry and Unity-oriented volume stubs.

use crate::mesh::Mesh;
use crate::smoke::SmokeGrid;
use serde::{Deserialize, Serialize};
use std::io::Write;

/// Result of an export operation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExportResult {
    pub path: String,
    pub vertex_count: u32,
    pub triangle_count: u32,
    pub byte_len: usize,
}

/// Unity-oriented smoke volume export metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmokeVolumeExport {
    pub path: String,
    pub format: String,
    pub nx: u32,
    pub ny: u32,
    pub nz: u32,
    pub frame_count: u32,
    pub byte_len: usize,
    pub notes: String,
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

/// Writes a raw little-endian f32 density volume for Unity import.
///
/// Format: 16-byte header (nx, ny, nz, frame_index as u32 LE) followed by
/// `nx*ny*nz` density samples in x-fastest order. Unity can load this into a
/// `Texture3D` or flipbook atlas via a small import script.
pub fn export_smoke_volume_raw(grid: &SmokeGrid, path: &str) -> std::io::Result<SmokeVolumeExport> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(&grid.nx.to_le_bytes())?;
    file.write_all(&grid.ny.to_le_bytes())?;
    file.write_all(&grid.nz.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?; // frame index
    for &d in &grid.density {
        file.write_all(&d.to_le_bytes())?;
    }
    let byte_len = 16 + grid.density.len() * 4;
    Ok(SmokeVolumeExport {
        path: path.to_string(),
        format: "elfentier_smoke_v1".into(),
        nx: grid.nx,
        ny: grid.ny,
        nz: grid.nz,
        frame_count: 1,
        byte_len,
        notes: "Raw f32 density grid (x-fastest). Header: u32 nx, ny, nz, frame. \
                Map to Texture3D or pack frames into a 2D flipbook atlas in Unity."
            .into(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::create_unit_box_mesh;
    use crate::smoke::{simulate_smoke, SmokeDomainParams, SmokeSolverParams, SmokeSourceParams};

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
    fn exports_smoke_volume_raw() {
        let grid = simulate_smoke(
            &SmokeDomainParams::default(),
            &SmokeSourceParams::default(),
            &SmokeSolverParams {
                steps: 2,
                ..Default::default()
            },
        );
        let path = std::env::temp_dir()
            .join("elfentier_smoke_test.raw")
            .to_string_lossy()
            .to_string();
        let result = export_smoke_volume_raw(&grid, &path).expect("export smoke");
        assert_eq!(result.format, "elfentier_smoke_v1");
        assert!(result.byte_len > 16);
        let _ = std::fs::remove_file(path);
    }
}
