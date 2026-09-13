//! GPU-friendly 3D volume texture export for Unity and other engines.
//!
//! Phase 1 writes `elfentier_volume_texture_v1` (`.evol` files). OpenVDB fog
//! `.vdb` I/O lives in `openvdb_io`; use `tools/vdb_convert` to bridge formats.

use crate::smoke::SmokeVolume;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, Write};

/// Binary format identifier for 3D volume textures.
pub const VOLUME_TEXTURE_FORMAT: &str = "elfentier_volume_texture_v1";

/// File extension recommended for volume texture payloads.
pub const VOLUME_TEXTURE_EXTENSION: &str = "evol";

const MAGIC: &[u8; 4] = b"EFVT";
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 52;

/// Parsed header for a volume texture file (data follows immediately after).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolumeTextureHeader {
    pub nx: u32,
    pub ny: u32,
    pub nz: u32,
    pub frame_count: u32,
    pub channels: u32,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

impl VolumeTextureHeader {
    pub fn voxel_count(&self) -> usize {
        self.nx as usize * self.ny as usize * self.nz as usize
    }

    pub fn frame_voxel_count(&self) -> usize {
        self.voxel_count() * self.channels as usize
    }

    pub fn data_byte_len(&self) -> usize {
        self.frame_voxel_count() * self.frame_count as usize * 4
    }
}

/// Result of writing a volume texture file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeTextureExportResult {
    pub path: String,
    pub frame_count: u32,
    pub byte_len: usize,
    pub format: String,
    pub resolution: [u32; 3],
}

/// Writes a 3D volume texture from smoke simulation density grids.
pub fn export_volume_texture(volume: &SmokeVolume, path: &str) -> std::io::Result<VolumeTextureExportResult> {
    let density_frames = volume.density_frames();
    let frame_count = density_frames.len().max(1) as u32;
    let res = volume.stats.resolution;
    let header = VolumeTextureHeader {
        nx: res[0],
        ny: res[1],
        nz: res[2],
        frame_count,
        channels: 1,
        bounds_min: [volume.bounds_min.x, volume.bounds_min.y, volume.bounds_min.z],
        bounds_max: [volume.bounds_max.x, volume.bounds_max.y, volume.bounds_max.z],
    };

    let mut file = std::fs::File::create(path)?;
    write_header(&mut file, &header)?;

    if density_frames.is_empty() {
        let zeros = vec![0.0_f32; header.frame_voxel_count()];
        write_f32_payload(&mut file, &zeros)?;
    } else {
        for frame in density_frames {
            write_f32_payload(&mut file, frame)?;
        }
    }

    let byte_len = HEADER_SIZE + header.data_byte_len();
    Ok(VolumeTextureExportResult {
        path: path.to_string(),
        frame_count,
        byte_len,
        format: VOLUME_TEXTURE_FORMAT.into(),
        resolution: res,
    })
}

/// Reads only the header from a volume texture file.
pub fn read_volume_texture_header(path: &str) -> std::io::Result<VolumeTextureHeader> {
    let mut file = std::fs::File::open(path)?;
    read_header(&mut file)
}

/// Reads all density samples for one frame (channel-major flattened x-fastest).
pub fn read_volume_texture_frame(path: &str, frame_index: u32) -> std::io::Result<Vec<f32>> {
    let mut file = std::fs::File::open(path)?;
    let header = read_header(&mut file)?;
    if frame_index >= header.frame_count {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "frame_index out of range",
        ));
    }
    let frame_bytes = header.frame_voxel_count() * 4;
    let offset = HEADER_SIZE + frame_index as usize * frame_bytes;
    file.seek(std::io::SeekFrom::Start(offset as u64))?;
    let mut buf = vec![0u8; frame_bytes];
    file.read_exact(&mut buf)?;
    Ok(bytes_to_f32(&buf))
}

fn write_header(writer: &mut impl Write, header: &VolumeTextureHeader) -> std::io::Result<()> {
    writer.write_all(MAGIC)?;
    writer.write_all(&VERSION.to_le_bytes())?;
    writer.write_all(&header.nx.to_le_bytes())?;
    writer.write_all(&header.ny.to_le_bytes())?;
    writer.write_all(&header.nz.to_le_bytes())?;
    writer.write_all(&header.frame_count.to_le_bytes())?;
    writer.write_all(&header.channels.to_le_bytes())?;
    for v in &header.bounds_min {
        writer.write_all(&v.to_le_bytes())?;
    }
    for v in &header.bounds_max {
        writer.write_all(&v.to_le_bytes())?;
    }
    Ok(())
}

fn read_header(reader: &mut impl Read) -> std::io::Result<VolumeTextureHeader> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid volume texture magic",
        ));
    }
    let version = read_u32(reader)?;
    if version != VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unsupported volume texture version {version}"),
        ));
    }
    Ok(VolumeTextureHeader {
        nx: read_u32(reader)?,
        ny: read_u32(reader)?,
        nz: read_u32(reader)?,
        frame_count: read_u32(reader)?,
        channels: read_u32(reader)?,
        bounds_min: [read_f32(reader)?, read_f32(reader)?, read_f32(reader)?],
        bounds_max: [read_f32(reader)?, read_f32(reader)?, read_f32(reader)?],
    })
}

fn write_f32_payload(writer: &mut impl Write, data: &[f32]) -> std::io::Result<()> {
    for value in data {
        writer.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}

fn read_u32(reader: &mut impl Read) -> std::io::Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_f32(reader: &mut impl Read) -> std::io::Result<f32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(f32::from_le_bytes(buf))
}

fn bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Builds a 2D density atlas by max-projecting each XY column through Z (flipbook-friendly).
pub fn build_density_atlas_xy(volume: &SmokeVolume) -> Vec<f32> {
    let density_frames = volume.density_frames();
    let res = volume.stats.resolution;
    let nx = res[0] as usize;
    let ny = res[1] as usize;
    let nz = res[2] as usize;
    let slice_cells = nx * ny;
    let frame_count = density_frames.len().max(1);
    let mut atlas = vec![0.0_f32; frame_count * slice_cells];

    if density_frames.is_empty() {
        return atlas;
    }

    for (frame_idx, density) in density_frames.iter().enumerate() {
        let base = frame_idx * slice_cells;
        for j in 0..ny {
            for i in 0..nx {
                let mut max_d = 0.0_f32;
                for k in 0..nz {
                    let idx = i + nx * (j + ny * k);
                    if idx < density.len() {
                        max_d = max_d.max(density[idx]);
                    }
                }
                atlas[base + i + nx * j] = max_d;
            }
        }
    }
    atlas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smoke::{
        simulate_smoke, SmokeDomainInput, SmokeSolverInput, SmokeSourceInput,
    };

    #[test]
    fn round_trip_volume_texture_header_and_frame() {
        let domain = SmokeDomainInput {
            resolution: 8,
            seed: 3,
            ..Default::default()
        };
        let volume = simulate_smoke(
            &domain,
            &[SmokeSourceInput::default()],
            &SmokeSolverInput {
                steps: 8,
                frame_stride: 4,
                ..Default::default()
            },
        );
        assert!(!volume.density_frames().is_empty());

        let path = std::env::temp_dir().join("elfentier_test_volume.evol");
        let path_str = path.to_string_lossy().to_string();
        let result = export_volume_texture(&volume, &path_str).expect("export");
        assert_eq!(result.format, VOLUME_TEXTURE_FORMAT);
        assert!(result.byte_len > HEADER_SIZE);

        let header = read_volume_texture_header(&path_str).expect("header");
        assert_eq!(header.nx, 8);
        assert_eq!(header.frame_count, volume.density_frames().len() as u32);

        let frame0 = read_volume_texture_frame(&path_str, 0).expect("frame");
        assert_eq!(frame0.len(), header.frame_voxel_count());
        assert!(frame0.iter().any(|&v| v > 0.0));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn atlas_projection_has_nonzero_density() {
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
        );
        let atlas = build_density_atlas_xy(&volume);
        assert!(atlas.iter().any(|&v| v > 0.0));
    }
}
