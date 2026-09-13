//! Volume conversion CLI — `.evol` interchange and OpenVDB fog I/O (Alpha 2).

use elfentier_core::graph::{evaluate_smoke_volume, Graph};
use elfentier_core::openvdb_io::{
    export_openvdb_fog, read_openvdb_density, read_openvdb_info, smoke_volume_from_openvdb,
    OPENVDB_FOG_FORMAT, VDB_EXTENSION,
};
use elfentier_core::volume_texture::{
    export_volume_texture, read_volume_texture_header, VOLUME_TEXTURE_EXTENSION,
    VOLUME_TEXTURE_FORMAT,
};
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "--help" | "-h" => {
            print_usage();
        }
        "info" => {
            let path = require_path(&args, 2);
            if is_vdb_path(path) {
                match read_openvdb_info(path) {
                    Ok(header) => {
                        println!("format: {}", OPENVDB_FOG_FORMAT);
                        println!("file_version: {}", header.file_version);
                        println!("grid: {}", header.grid_name);
                        println!("grid_type: {}", header.grid_type);
                        println!(
                            "index_bbox_min: [{}, {}, {}]",
                            header.bounds_min[0],
                            header.bounds_min[1],
                            header.bounds_min[2]
                        );
                        println!(
                            "index_bbox_max: [{}, {}, {}]",
                            header.bounds_max[0],
                            header.bounds_max[1],
                            header.bounds_max[2]
                        );
                        println!("active_voxels: {}", header.voxel_count);
                        println!("data_bytes: {}", header.byte_len);
                    }
                    Err(err) => {
                        eprintln!("error: {err}");
                        std::process::exit(1);
                    }
                }
            } else {
                match read_volume_texture_header(path) {
                    Ok(header) => {
                        println!("format: {}", VOLUME_TEXTURE_FORMAT);
                        println!(
                            "resolution: {}x{}x{}",
                            header.nx,
                            header.ny,
                            header.nz
                        );
                        println!("frames: {}", header.frame_count);
                        println!("channels: {}", header.channels);
                        println!(
                            "bounds_min: [{:.3}, {:.3}, {:.3}]",
                            header.bounds_min[0],
                            header.bounds_min[1],
                            header.bounds_min[2]
                        );
                        println!(
                            "bounds_max: [{:.3}, {:.3}, {:.3}]",
                            header.bounds_max[0],
                            header.bounds_max[1],
                            header.bounds_max[2]
                        );
                        println!("data_bytes: {}", header.data_byte_len());
                    }
                    Err(err) => {
                        eprintln!("error: {err}");
                        std::process::exit(1);
                    }
                }
            }
        }
        "from-smoke-preset" => {
            let output = require_path(&args, 2);
            let graph = Graph::smoke_puff_preset();
            let volume = evaluate_smoke_volume(&graph).expect("cook smoke preset");
            let result = export_volume_texture(&volume, output).expect("write volume");
            println!(
                "wrote {} ({} frames, {} bytes)",
                result.path,
                result.frame_count,
                result.byte_len
            );
        }
        "to-vdb" => {
            let output = require_path(&args, 2);
            if args.len() >= 4 && args[3] == "--from-preset" {
                let graph = Graph::smoke_puff_preset();
                let volume = evaluate_smoke_volume(&graph).expect("cook smoke preset");
                let result = export_openvdb_fog(&volume, output).expect("write vdb");
                println!(
                    "wrote {} (grid={}, active_voxels={}, {} bytes)",
                    result.path,
                    result.grid_name,
                    result.active_voxels,
                    result.byte_len
                );
            } else {
                let input = require_path(&args, 3);
                if input.ends_with(".evol") {
                    eprintln!(
                        "`.evol` to `.vdb` conversion requires a cooked smoke graph today.\n\
                         Use: vdb_convert to-vdb <out.vdb> --from-preset\n\
                         Or export a bundle from elfentierFX (includes smoke_density.vdb)."
                    );
                    std::process::exit(2);
                }
                eprintln!("unknown input `{input}` for to-vdb");
                std::process::exit(1);
            }
        }
        "from-vdb" => {
            let input = require_path(&args, 2);
            let output = require_path(&args, 3);
            if is_vdb_path(input) {
                if output.ends_with(".evol") {
                    let volume = smoke_volume_from_openvdb(input, None).expect("read vdb");
                    let result = export_volume_texture(&volume, output).expect("write evol");
                    println!(
                        "wrote {} ({} frames, {} bytes)",
                        result.path,
                        result.frame_count,
                        result.byte_len
                    );
                } else {
                    let grid = read_openvdb_density(input, None).expect("read vdb");
                    println!(
                        "grid={} resolution={}x{}x{} samples={}",
                        grid.grid_name,
                        grid.resolution[0],
                        grid.resolution[1],
                        grid.resolution[2],
                        grid.density.len()
                    );
                    println!("(no output path with supported extension; use <out.evol>)");
                    std::process::exit(1);
                }
            } else {
                eprintln!("expected `.vdb` input for from-vdb");
                std::process::exit(1);
            }
        }
        other => {
            let input = other;
            let _output = require_path(&args, 2);
            if is_vdb_path(input) {
                eprintln!(
                    "`.vdb` input detected. Use `vdb_convert from-vdb <file.vdb> <out.evol>` \
                     or `vdb_convert info <file.vdb>`."
                );
                std::process::exit(2);
            }
            if input.ends_with(".evol") {
                eprintln!("input already appears to be .evol; use `vdb_convert info <path>`");
                std::process::exit(1);
            }
            eprintln!("unknown input `{input}` — expected .vdb or a subcommand");
            print_usage();
            std::process::exit(1);
        }
    }
}

fn is_vdb_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(VDB_EXTENSION))
        .unwrap_or(false)
}

fn require_path(args: &[String], index: usize) -> &str {
    match args.get(index) {
        Some(path) if !path.is_empty() => path.as_str(),
        _ => {
            eprintln!("missing path argument at position {}", index);
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!(
        "vdb_convert — elfentier volume interchange + OpenVDB fog I/O (Alpha 2)\n\
         \n\
         Usage:\n\
           vdb_convert info <file.vdb|file.evol>\n\
           vdb_convert from-smoke-preset <out.evol>\n\
           vdb_convert to-vdb <out.vdb> --from-preset\n\
           vdb_convert from-vdb <file.vdb> <out.evol>\n\
         \n\
         Formats:\n\
           {vdb_format} (.{vdb_ext}) — OpenVDB FloatGrid fog volume (write + read)\n\
           {evol_format} (.{evol_ext}) — elfentier 3D texture interchange\n\
         \n\
         OpenVDB files use uncompressed active-mask encoding and are readable by\n\
         vdb-rs and other standard OpenVDB-compatible tools.\n\
         Unity users can also install Unity Volume Importer for legacy .evol bundles.\n\
         \n\
         OpenVDB is a trademark of LF Projects, LLC.",
        vdb_format = OPENVDB_FOG_FORMAT,
        vdb_ext = VDB_EXTENSION,
        evol_format = VOLUME_TEXTURE_FORMAT,
        evol_ext = VOLUME_TEXTURE_EXTENSION
    );
}
