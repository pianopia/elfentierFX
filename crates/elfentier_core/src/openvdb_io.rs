//! Minimal OpenVDB fog-volume I/O for smoke density grids.
//!
//! Writes standard `.vdb` FloatGrid files (uncompressed, readable by `vdb-rs` and
//! OpenVDB-compatible tools). Reading uses the pure-Rust `vdb-rs` parser.

use crate::mesh::Vec3;
use crate::smoke::SmokeVolume;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;

/// Payload format identifier for exported OpenVDB fog grids.
pub const OPENVDB_FOG_FORMAT: &str = "openvdb_fog_floatgrid_v1";

/// Recommended file extension for OpenVDB archives.
pub const VDB_EXTENSION: &str = "vdb";

const MAGIC: u64 = 0x0000000056444220;
const FILE_VERSION: u32 = 224;
const LIB_MAJOR: u32 = 12;
const LIB_MINOR: u32 = 1;
const GRID_TYPE: &str = "Tree_float_5_4_3";
const LEAF_DIM: i32 = 8;
const NODE4_STRIDE: i32 = 128;
/// Summary information from an on-disk `.vdb` archive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenVdbInfo {
    pub path: String,
    pub file_version: u32,
    pub grid_names: Vec<String>,
    pub grid_name: String,
    pub grid_type: String,
    pub bounds_min: [i32; 3],
    pub bounds_max: [i32; 3],
    pub voxel_count: i64,
    pub byte_len: usize,
}

/// Dense density grid reconstructed from a fog `.vdb` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenVdbDensityGrid {
    pub grid_name: String,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub resolution: [u32; 3],
    pub density: Vec<f32>,
}

/// Result of writing a fog `.vdb` file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenVdbExportResult {
    pub path: String,
    pub grid_name: String,
    pub frame_index: u32,
    pub byte_len: usize,
    pub format: String,
    pub active_voxels: u64,
    pub resolution: [u32; 3],
}

#[derive(Debug, Clone)]
struct LeafBlock {
    origin: [i32; 3],
    values: Vec<f32>,
    active: Vec<bool>,
}

#[derive(Debug, Clone)]
struct Node4Block {
    origin: [i32; 3],
    leaves: BTreeMap<u32, LeafBlock>,
}

#[derive(Debug, Clone)]
struct Node5Block {
    origin: [i32; 3],
    children: BTreeMap<u32, Node4Block>,
}

/// Writes the last smoke density frame as an OpenVDB fog FloatGrid (`.vdb`).
pub fn export_openvdb_fog(volume: &SmokeVolume, path: &str) -> std::io::Result<OpenVdbExportResult> {
    let frames = volume.density_frames();
    let frame_index = frames.len().saturating_sub(1);
    let density = frames
        .get(frame_index)
        .map(|frame| frame.as_slice())
        .unwrap_or(&[]);
    let res = volume.stats.resolution;
    export_openvdb_fog_grid(
        path,
        "density",
        res[0] as usize,
        res[1] as usize,
        res[2] as usize,
        [
            volume.bounds_min.x as f64,
            volume.bounds_min.y as f64,
            volume.bounds_min.z as f64,
        ],
        [
            volume.bounds_max.x as f64,
            volume.bounds_max.y as f64,
            volume.bounds_max.z as f64,
        ],
        density,
        frame_index,
    )
}

/// Writes a dense `nx × ny × nz` fog grid to a `.vdb` file.
pub fn export_openvdb_fog_grid(
    path: &str,
    grid_name: &str,
    nx: usize,
    ny: usize,
    nz: usize,
    bounds_min: [f64; 3],
    bounds_max: [f64; 3],
    density: &[f32],
    frame_index: usize,
) -> std::io::Result<OpenVdbExportResult> {
    let tree = build_tree_from_dense(nx, ny, nz, density);
    let (index_min, index_max, active_voxels) = tree_index_bounds(&tree);
    let voxel_count = active_voxels as i64;
    let mem_bytes = (active_voxels * 4 + 4096) as i64;

    let mut topology = Vec::new();
    write_tree_topology(&mut topology, &tree)?;
    let mut data = Vec::new();
    write_tree_data(&mut data, &tree)?;

    let mut grid_prefix = Vec::new();
    write_u32(&mut grid_prefix, 2)?; // Compression::ACTIVE_MASK (uncompressed)
    write_grid_metadata(
        &mut grid_prefix,
        grid_name,
        index_min,
        index_max,
        voxel_count,
        mem_bytes,
        [nx as u32, ny as u32, nz as u32],
    )?;
    write_transform(&mut grid_prefix, bounds_min, bounds_max, nx, ny, nz)?;

    let mut file = Vec::new();
    write_archive_header(&mut file)?;
    let grid_pos = file.len() as u64 + estimate_descriptor_size(grid_name);
    let block_pos = grid_pos + grid_prefix.len() as u64 + topology.len() as u64;
    let end_pos = block_pos + data.len() as u64;

    write_grid_descriptor(&mut file, grid_name, grid_pos, block_pos, end_pos)?;
    file.extend_from_slice(&grid_prefix);
    file.extend_from_slice(&topology);
    file.extend_from_slice(&data);

    std::fs::write(path, &file)?;
    Ok(OpenVdbExportResult {
        path: path.to_string(),
        grid_name: grid_name.to_string(),
        frame_index: frame_index as u32,
        byte_len: file.len(),
        format: OPENVDB_FOG_FORMAT.into(),
        active_voxels,
        resolution: [nx as u32, ny as u32, nz as u32],
    })
}

/// Reads summary metadata from a `.vdb` file.
pub fn read_openvdb_info(path: &str) -> std::io::Result<OpenVdbInfo> {
    let file = std::fs::File::open(path)?;
    let reader = vdb_rs::VdbReader::new(std::io::BufReader::new(file))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    let grid_names = reader.available_grids();
    let grid_name = grid_names
        .first()
        .cloned()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no grids in file"))?;
    let descriptor = reader
        .grid_descriptors
        .get(&grid_name)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "grid not found"))?
        .clone();
    let bounds_min = descriptor
        .aabb_min()
        .map(|v| [v.x, v.y, v.z])
        .unwrap_or([0, 0, 0]);
    let bounds_max = descriptor
        .aabb_max()
        .map(|v| [v.x, v.y, v.z])
        .unwrap_or([0, 0, 0]);
    let voxel_count = descriptor.voxel_count().unwrap_or(0);
    let byte_len = std::fs::metadata(path)?.len() as usize;
    Ok(OpenVdbInfo {
        path: path.to_string(),
        file_version: reader.header.file_version,
        grid_names,
        grid_name,
        grid_type: descriptor.grid_type,
        bounds_min,
        bounds_max,
        voxel_count,
        byte_len,
    })
}

/// Reconstructs a dense density grid from the first (or named) fog grid in a `.vdb` file.
pub fn read_openvdb_density(path: &str, grid_name: Option<&str>) -> std::io::Result<OpenVdbDensityGrid> {
    let file = std::fs::File::open(path)?;
    let mut reader = vdb_rs::VdbReader::new(std::io::BufReader::new(file))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    let available = reader.available_grids();
    let name = grid_name
        .map(|s| s.to_string())
        .or_else(|| available.first().cloned())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no grids in file"))?;
    let descriptor = reader.grid_descriptors.get(&name).cloned().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, format!("grid `{name}` not found"))
    })?;
    let grid = reader
        .read_grid::<f32>(&name)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let resolution = read_resolution_metadata(&descriptor.meta_data).unwrap_or_else(|| {
        let index_min = descriptor.aabb_min().map(|v| [v.x, v.y, v.z]).unwrap_or([0, 0, 0]);
        let index_max = descriptor.aabb_max().map(|v| [v.x, v.y, v.z]).unwrap_or([0, 0, 0]);
        [
            (index_max[0] - index_min[0] + 1).max(1) as u32,
            (index_max[1] - index_min[1] + 1).max(1) as u32,
            (index_max[2] - index_min[2] + 1).max(1) as u32,
        ]
    });
    let [nx, ny, nz] = resolution;
    let mut density = vec![0.0_f32; (nx as usize) * (ny as usize) * (nz as usize)];

    let (world_min, _world_max, voxel_size) = transform_world_bounds(&grid.transform);
    let world_max = [
        world_min[0] + voxel_size[0] * nx as f32,
        world_min[1] + voxel_size[1] * ny as f32,
        world_min[2] + voxel_size[2] * nz as f32,
    ];
    for (coord, value, _level) in grid.iter() {
        let ix = coord.x.round() as i32;
        let iy = coord.y.round() as i32;
        let iz = coord.z.round() as i32;
        if ix < 0 || iy < 0 || iz < 0 {
            continue;
        }
        let (ix, iy, iz) = (ix as u32, iy as u32, iz as u32);
        if ix >= nx || iy >= ny || iz >= nz {
            continue;
        }
        let idx = ix as usize + (nx as usize) * (iy as usize + (ny as usize) * iz as usize);
        density[idx] = value;
    }

    Ok(OpenVdbDensityGrid {
        grid_name: name,
        bounds_min: world_min,
        bounds_max: world_max,
        resolution: [nx, ny, nz],
        density,
    })
}

/// Imports a `.vdb` fog grid into a `SmokeVolume` shell for viewport / cook display.
pub fn smoke_volume_from_openvdb(path: &str, grid_name: Option<&str>) -> std::io::Result<SmokeVolume> {
    let grid = read_openvdb_density(path, grid_name)?;
    let frame = crate::smoke::SmokeFrame {
        positions: Vec::new(),
        sizes: Vec::new(),
        opacities: Vec::new(),
        particle_count: 0,
    };
    let max_density = grid.density.iter().copied().fold(0.0_f32, f32::max);
    let total_density = grid.density.iter().sum();
    Ok(SmokeVolume {
        frames: vec![frame],
        density_grids: vec![grid.density],
        bounds_min: Vec3::new(grid.bounds_min[0], grid.bounds_min[1], grid.bounds_min[2]),
        bounds_max: Vec3::new(grid.bounds_max[0], grid.bounds_max[1], grid.bounds_max[2]),
        stats: crate::smoke::SmokeStats {
            resolution: grid.resolution,
            max_density,
            step_count: 0,
            frame_count: 1,
            particle_count: 0,
            total_density,
        },
    })
}

fn read_resolution_metadata(meta: &vdb_rs::Metadata) -> Option<[u32; 3]> {
    let nx = meta.0.get("elfentier_nx").and_then(|v| match v {
        vdb_rs::MetadataValue::I32(n) => Some(*n as u32),
        _ => None,
    });
    let ny = meta.0.get("elfentier_ny").and_then(|v| match v {
        vdb_rs::MetadataValue::I32(n) => Some(*n as u32),
        _ => None,
    });
    let nz = meta.0.get("elfentier_nz").and_then(|v| match v {
        vdb_rs::MetadataValue::I32(n) => Some(*n as u32),
        _ => None,
    });
    match (nx, ny, nz) {
        (Some(nx), Some(ny), Some(nz)) => Some([nx, ny, nz]),
        _ => None,
    }
}

fn transform_world_bounds(transform: &vdb_rs::Map) -> ([f32; 3], [f32; 3], [f32; 3]) {
    match transform {
        vdb_rs::Map::ScaleTranslateMap {
            translation,
            voxel_size,
            ..
        } => {
            let origin = [
                translation.x as f32,
                translation.y as f32,
                translation.z as f32,
            ];
            let vs = [
                voxel_size.x as f32,
                voxel_size.y as f32,
                voxel_size.z as f32,
            ];
            (
                origin,
                [origin[0] + vs[0], origin[1] + vs[1], origin[2] + vs[2]],
                vs,
            )
        }
        vdb_rs::Map::UniformScaleMap { voxel_size, .. } => {
            let vs = [
                voxel_size.x as f32,
                voxel_size.y as f32,
                voxel_size.z as f32,
            ];
            ([0.0, 0.0, 0.0], vs, vs)
        }
    }
}

fn build_tree_from_dense(nx: usize, ny: usize, nz: usize, density: &[f32]) -> Node5Block {
    let mut root = Node5Block {
        origin: [0, 0, 0],
        children: BTreeMap::new(),
    };

    let sample = |x: usize, y: usize, z: usize| -> f32 {
        if x >= nx || y >= ny || z >= nz {
            0.0
        } else {
            let idx = x + nx * (y + ny * z);
            density.get(idx).copied().unwrap_or(0.0)
        }
    };

    for bz in (0..nz).step_by(LEAF_DIM as usize) {
        for by in (0..ny).step_by(LEAF_DIM as usize) {
            for bx in (0..nx).step_by(LEAF_DIM as usize) {
                let origin = [bx as i32, by as i32, bz as i32];
                let mut values = Vec::with_capacity(512);
                let mut active = Vec::with_capacity(512);
                let mut has_active = false;
                for lz in 0..LEAF_DIM {
                    for ly in 0..LEAF_DIM {
                        for lx in 0..LEAF_DIM {
                            let value = sample(bx + lx as usize, by + ly as usize, bz + lz as usize);
                            let is_active = value > 1.0e-7;
                            values.push(value);
                            active.push(is_active);
                            has_active |= is_active;
                        }
                    }
                }
                if !has_active {
                    continue;
                }
                let leaf = LeafBlock {
                    origin,
                    values,
                    active,
                };
                let node4_origin = align_origin(origin, NODE4_STRIDE);
                let node4_offset = node_offset(node4_origin, root.origin, 5);
                let node4 = root.children.entry(node4_offset).or_insert_with(|| Node4Block {
                    origin: node4_origin,
                    leaves: BTreeMap::new(),
                });
                let leaf_offset = node_offset(origin, node4.origin, 4);
                node4.leaves.insert(leaf_offset, leaf);
            }
        }
    }
    root
}

fn tree_index_bounds(tree: &Node5Block) -> ([i32; 3], [i32; 3], u64) {
    let mut min = [i32::MAX, i32::MAX, i32::MAX];
    let mut max = [i32::MIN, i32::MIN, i32::MIN];
    let mut active = 0u64;
    for node4 in tree.children.values() {
        for leaf in node4.leaves.values() {
            for (idx, is_active) in leaf.active.iter().enumerate() {
                if !*is_active {
                    continue;
                }
                active += 1;
                let (lx, ly, lz) = offset_to_local(idx as u32, 3);
                let gx = leaf.origin[0] + (lx << 0);
                let gy = leaf.origin[1] + (ly << 0);
                let gz = leaf.origin[2] + (lz << 0);
                min[0] = min[0].min(gx);
                min[1] = min[1].min(gy);
                min[2] = min[2].min(gz);
                max[0] = max[0].max(gx);
                max[1] = max[1].max(gy);
                max[2] = max[2].max(gz);
            }
        }
    }
    if active == 0 {
        return ([0, 0, 0], [-1, -1, -1], 0);
    }
    (min, max, active)
}

fn align_origin(origin: [i32; 3], stride: i32) -> [i32; 3] {
    [
        origin[0] & !(stride - 1),
        origin[1] & !(stride - 1),
        origin[2] & !(stride - 1),
    ]
}

fn node_offset(origin: [i32; 3], parent_origin: [i32; 3], level: u32) -> u32 {
    let shift = if level == 5 { 7 } else { 3 };
    let lx = (origin[0] - parent_origin[0]) >> shift;
    let ly = (origin[1] - parent_origin[1]) >> shift;
    let lz = (origin[2] - parent_origin[2]) >> shift;
    pack_offset(lx, ly, lz, if level == 5 { 5 } else { 4 })
}

fn pack_offset(x: i32, y: i32, z: i32, log2dim: u32) -> u32 {
    ((x as u32) << (2 * log2dim)) | ((y as u32) << log2dim) | (z as u32)
}

fn offset_to_local(offset: u32, log2dim: u32) -> (i32, i32, i32) {
    let x = (offset >> (2 * log2dim)) as i32;
    let rem = offset & ((1 << (2 * log2dim)) - 1);
    let y = (rem >> log2dim) as i32;
    let z = (rem & ((1 << log2dim) - 1)) as i32;
    (x, y, z)
}

fn write_archive_header(writer: &mut Vec<u8>) -> std::io::Result<()> {
    write_u64(writer, MAGIC)?;
    write_u32(writer, FILE_VERSION)?;
    write_u32(writer, LIB_MAJOR)?;
    write_u32(writer, LIB_MINOR)?;
    writer.write_all(&[1u8])?; // has_grid_offsets
    writer.write_all(b"00000000-0000-4000-8000-000000000001")?;
    write_metadata(writer, &[])?;
    write_u32(writer, 1)?; // grid_count
    Ok(())
}

fn estimate_descriptor_size(grid_name: &str) -> u64 {
    let base = 4 + grid_name.len() + 4 + GRID_TYPE.len() + 4 + 24;
    base as u64
}

fn write_grid_descriptor(
    writer: &mut Vec<u8>,
    grid_name: &str,
    grid_pos: u64,
    block_pos: u64,
    end_pos: u64,
) -> std::io::Result<()> {
    write_name(writer, grid_name)?;
    write_name(writer, GRID_TYPE)?;
    write_name(writer, "")?;
    write_u64(writer, grid_pos)?;
    write_u64(writer, block_pos)?;
    write_u64(writer, end_pos)?;
    Ok(())
}

fn write_grid_metadata(
    writer: &mut Vec<u8>,
    grid_name: &str,
    index_min: [i32; 3],
    index_max: [i32; 3],
    voxel_count: i64,
    mem_bytes: i64,
    resolution: [u32; 3],
) -> std::io::Result<()> {
    let nx_bytes = (resolution[0] as i32).to_le_bytes();
    let ny_bytes = (resolution[1] as i32).to_le_bytes();
    let nz_bytes = (resolution[2] as i32).to_le_bytes();
    let entries: [(&str, &str, &[u8]); 12] = [
        ("class", "string", b"fog volume"),
        ("file_bbox_min", "vec3i", &pack_i32x3(index_min)),
        ("file_bbox_max", "vec3i", &pack_i32x3(index_max)),
        ("file_compression", "string", b"active values"),
        ("file_mem_bytes", "int64", &pack_i64(mem_bytes)),
        ("file_voxel_count", "int64", &pack_i64(voxel_count)),
        ("is_saved_as_half_float", "bool", &[0]),
        ("name", "string", grid_name.as_bytes()),
        ("value_type", "string", b"float"),
        ("elfentier_nx", "int32", &nx_bytes),
        ("elfentier_ny", "int32", &ny_bytes),
        ("elfentier_nz", "int32", &nz_bytes),
    ];
    write_u32(writer, entries.len() as u32)?;
    for (name, dtype, payload) in entries {
        write_metadata_entry(writer, name, dtype, payload)?;
    }
    Ok(())
}

fn write_transform(
    writer: &mut Vec<u8>,
    bounds_min: [f64; 3],
    bounds_max: [f64; 3],
    nx: usize,
    ny: usize,
    nz: usize,
) -> std::io::Result<()> {
    let voxel_size = [
        (bounds_max[0] - bounds_min[0]) / nx.max(1) as f64,
        (bounds_max[1] - bounds_min[1]) / ny.max(1) as f64,
        (bounds_max[2] - bounds_min[2]) / nz.max(1) as f64,
    ];
    let scale = [1.0, 1.0, 1.0];
    let inv_scale = [1.0, 1.0, 1.0];
    let inv_scale_sqr = [1.0, 1.0, 1.0];
    let inv_twice_scale = [0.5, 0.5, 0.5];

    write_name(writer, "ScaleTranslateMap")?;
    write_f64x3(writer, bounds_min)?;
    write_f64x3(writer, scale)?;
    write_f64x3(writer, voxel_size)?;
    write_f64x3(writer, inv_scale)?;
    write_f64x3(writer, inv_scale_sqr)?;
    write_f64x3(writer, inv_twice_scale)?;
    Ok(())
}

fn write_tree_topology(writer: &mut Vec<u8>, tree: &Node5Block) -> std::io::Result<()> {
    write_u32(writer, 1)?; // buffer_count
    write_f32(writer, 0.0)?; // background
    write_u32(writer, 0)?; // tile_count
    write_u32(writer, 1)?; // root_count
    write_i32x3(writer, tree.origin)?;
    write_internal_node_header(writer, 5, &child_mask_node5(tree))?;
    for node4 in tree.children.values() {
        write_internal_node_header(writer, 4, &child_mask_node4(node4))?;
        for leaf in node4.leaves.values() {
            write_bitmask(writer, &leaf.active, 512)?;
        }
    }
    Ok(())
}

fn write_tree_data(writer: &mut Vec<u8>, tree: &Node5Block) -> std::io::Result<()> {
    for node4 in tree.children.values() {
        for leaf in node4.leaves.values() {
            write_bitmask(writer, &leaf.active, 512)?;
            write_leaf_buffer(writer, leaf)?;
        }
    }
    Ok(())
}

fn child_mask_node5(tree: &Node5Block) -> Vec<bool> {
    let mut mask = vec![false; 32768];
    for offset in tree.children.keys() {
        mask[*offset as usize] = true;
    }
    mask
}

fn child_mask_node4(node4: &Node4Block) -> Vec<bool> {
    let mut mask = vec![false; 4096];
    for offset in node4.leaves.keys() {
        mask[*offset as usize] = true;
    }
    mask
}

fn write_internal_node_header(
    writer: &mut Vec<u8>,
    log2dim: u32,
    child_mask: &[bool],
) -> std::io::Result<()> {
    let total_bits = 1usize << (3 * log2dim);
    let value_mask = vec![false; total_bits];
    write_bitmask(writer, child_mask, total_bits)?;
    write_bitmask(writer, &value_mask, total_bits)?;
    writer.write_all(&[0u8])?; // NodeMetaData::NoMaskOrInactiveVals — child-only internal nodes
    Ok(())
}

fn write_leaf_buffer(writer: &mut Vec<u8>, leaf: &LeafBlock) -> std::io::Result<()> {
    writer.write_all(&[6u8])?; // NodeMetaData::NoMaskAndAllVals
    for (idx, value) in leaf.values.iter().enumerate() {
        let stored = if leaf.active[idx] { *value } else { 0.0 };
        write_f32(writer, stored)?;
    }
    Ok(())
}

fn write_metadata(writer: &mut Vec<u8>, entries: &[(&str, &str, &[u8])]) -> std::io::Result<()> {
    write_u32(writer, entries.len() as u32)?;
    for (name, dtype, payload) in entries {
        write_metadata_entry(writer, name, dtype, payload)?;
    }
    Ok(())
}

fn write_metadata_entry(
    writer: &mut Vec<u8>,
    name: &str,
    dtype: &str,
    payload: &[u8],
) -> std::io::Result<()> {
    write_name(writer, name)?;
    write_name(writer, dtype)?;
    write_u32(writer, payload.len() as u32)?;
    writer.write_all(payload)?;
    Ok(())
}

fn write_name(writer: &mut Vec<u8>, value: &str) -> std::io::Result<()> {
    write_u32(writer, value.len() as u32)?;
    writer.write_all(value.as_bytes())?;
    Ok(())
}

fn write_bitmask(writer: &mut Vec<u8>, bits: &[bool], total_bits: usize) -> std::io::Result<()> {
    let words = total_bits / 64;
    for word in 0..words {
        let mut value = 0u64;
        for bit in 0..64 {
            let idx = word * 64 + bit;
            if idx < bits.len() && bits[idx] {
                value |= 1u64 << bit;
            }
        }
        write_u64(writer, value)?;
    }
    Ok(())
}

fn pack_i32x3(v: [i32; 3]) -> [u8; 12] {
    let mut out = [0u8; 12];
    for (i, item) in v.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&item.to_le_bytes());
    }
    out
}

fn pack_i64(v: i64) -> [u8; 8] {
    v.to_le_bytes()
}

fn write_u32(writer: &mut Vec<u8>, value: u32) -> std::io::Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u64(writer: &mut Vec<u8>, value: u64) -> std::io::Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_f32(writer: &mut Vec<u8>, value: f32) -> std::io::Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_f64x3(writer: &mut Vec<u8>, values: [f64; 3]) -> std::io::Result<()> {
    for value in values {
        writer.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}

fn write_i32x3(writer: &mut Vec<u8>, values: [i32; 3]) -> std::io::Result<()> {
    for value in values {
        writer.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smoke::{
        simulate_smoke, SmokeDomainInput, SmokeSolverInput, SmokeSourceInput,
    };

    #[test]
    fn write_and_read_openvdb_round_trip() {
        let domain = SmokeDomainInput {
            resolution: 12,
            seed: 11,
            ..Default::default()
        };
        let volume = simulate_smoke(
            &domain,
            &[SmokeSourceInput::default()],
            &SmokeSolverInput {
                steps: 6,
                frame_stride: 3,
                ..Default::default()
            },
            &[],
        );
        let path = std::env::temp_dir().join("elfentier_test_smoke.vdb");
        let path_str = path.to_string_lossy().to_string();
        let export = export_openvdb_fog(&volume, &path_str).expect("export vdb");
        assert!(export.active_voxels > 0);
        assert!(std::path::Path::new(&path_str).exists());

        let info = read_openvdb_info(&path_str).expect("info");
        assert_eq!(info.grid_name, "density");
        assert!(info.voxel_count > 0);

        let grid = read_openvdb_density(&path_str, None).expect("read density");
        assert_eq!(grid.resolution, export.resolution);
        let source = volume.density_frames().last().expect("source frame");
        let mut matched = 0usize;
        for (idx, &value) in source.iter().enumerate() {
            if value <= 1.0e-7 {
                continue;
            }
            let read_back = grid.density.get(idx).copied().unwrap_or(0.0);
            if (read_back - value).abs() < 1.0e-5 {
                matched += 1;
            }
        }
        assert!(matched > 0, "expected at least one active voxel to round-trip");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn openvdb_info_reports_byte_len() {
        let domain = SmokeDomainInput {
            resolution: 8,
            ..Default::default()
        };
        let volume = simulate_smoke(
            &domain,
            &[SmokeSourceInput::default()],
            &SmokeSolverInput {
                steps: 4,
                frame_stride: 2,
                ..Default::default()
            },
            &[],
        );
        let path = std::env::temp_dir().join("elfentier_test_info.vdb");
        let path_str = path.to_string_lossy().to_string();
        export_openvdb_fog(&volume, &path_str).expect("export");
        let info = read_openvdb_info(&path_str).expect("info");
        assert!(info.byte_len > 256);
        let _ = std::fs::remove_file(path);
    }
}
