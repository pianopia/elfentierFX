//! Conversion of actual interchange volumes, including UE-native VDB sequences.
use elfentier_core::openvdb_io::export_openvdb_fog_grid;
use elfentier_core::volume_texture::{read_volume_texture_frame, read_volume_texture_header};
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

fn invalid(message: &str) -> Error {
    Error::new(ErrorKind::InvalidData, message)
}

/// Converts every frame with --sequence; otherwise preserves the existing last-frame convention.
/// UE space uses centimeters, Z-up, and X/Z/Y axis permutation matching glTF Interchange.
pub fn convert_evol(
    input: &Path,
    output: &Path,
    sequence: bool,
    ue_space: bool,
) -> Result<Vec<PathBuf>> {
    let input_str = input.to_str().ok_or_else(|| invalid("Non-UTF8 input path"))?;
    let header = read_volume_texture_header(input_str)?;
    let source_res = [header.nx as usize, header.ny as usize, header.nz as usize];
    if source_res.iter().any(|&n| n == 0 || n > 512)
        || header.channels != 1
        || header.frame_count == 0
        || header.frame_count > 10000
    {
        return Err(invalid(
            "Expected scalar density, nonzero frames and 1..512 resolution",
        ));
    }
    let cells = source_res.iter().product::<usize>();
    let data_bytes = cells
        .checked_mul(header.frame_count as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| invalid("Volume size overflow"))?;
    if data_bytes > 512 * 1024 * 1024 {
        return Err(invalid("Volume exceeds 512 MiB"));
    }
    if std::fs::metadata(input)?.len() != 52 + data_bytes as u64 {
        return Err(invalid("Truncated or trailing volume data"));
    }
    for axis in 0..3 {
        if !header.bounds_min[axis].is_finite()
            || !header.bounds_max[axis].is_finite()
            || header.bounds_max[axis] <= header.bounds_min[axis]
        {
            return Err(invalid("Invalid volume bounds"));
        }
    }
    let frames: Vec<u32> = if sequence {
        (0..header.frame_count).collect()
    } else {
        vec![header.frame_count - 1]
    };
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("Invalid output filename"))?;
    let paths: Vec<PathBuf> = frames
        .iter()
        .map(|i| {
            if sequence {
                output.with_file_name(format!("{stem}_{i:04}.vdb"))
            } else {
                output.to_path_buf()
            }
        })
        .collect();
    if paths.iter().any(|p| p.exists()) {
        return Err(Error::new(ErrorKind::AlreadyExists, "VDB outputs already exist"));
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let order = if ue_space { [0, 2, 1] } else { [0, 1, 2] };
    let scale = if ue_space { 100.0 } else { 1.0 };
    let res = order.map(|a| source_res[a]);
    let min = order.map(|a| header.bounds_min[a] as f64 * scale);
    let max = order.map(|a| header.bounds_max[a] as f64 * scale);
    for (&frame, path) in frames.iter().zip(paths.iter()) {
        let source = read_volume_texture_frame(input_str, frame)?;
        if source.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(invalid("Invalid fog density"));
        }
        let mut density = vec![0.0; cells];
        for z in 0..source_res[2] {
            for y in 0..source_res[1] {
                for x in 0..source_res[0] {
                    let xyz = [x, y, z];
                    let target = order.map(|a| xyz[a]);
                    let index = target[0] + res[0] * (target[1] + res[1] * target[2]);
                    density[index] = source[x + source_res[0] * (y + source_res[1] * z)];
                }
            }
        }
        export_openvdb_fog_grid(
            path.to_str().ok_or_else(|| invalid("Non-UTF8 output path"))?,
            "density", res[0], res[1], res[2], min, max, &density, frame as usize,
        )?;
    }
    Ok(paths)
}
