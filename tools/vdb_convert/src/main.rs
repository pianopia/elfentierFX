//! Phase 1 volume conversion CLI for Unity and other engines.
//!
//! Native OpenVDB `.vdb` read is not wired in this milestone — use elfentierFX
//! bundle export or smoke cook output, then convert to `elfentier_volume_texture_v1`.

use elfentier_core::graph::{evaluate_smoke_volume, Graph};
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
        "from-vdb" => {
            let _input = require_path(&args, 2);
            eprintln!(
                "Phase 1: native .vdb import is not available in this build.\n\
                 Roadmap: link OpenVDB/NanoVDB reader here, emit .{ext} ({format}).\n\
                 Workaround: export a bundle from elfentierFX desktop (volume_texture.evol) \
                 or run `vdb_convert from-smoke-preset <out.evol>`.",
                ext = VOLUME_TEXTURE_EXTENSION,
                format = VOLUME_TEXTURE_FORMAT
            );
            std::process::exit(2);
        }
        other => {
            let input = other;
            let _output = require_path(&args, 2);
            if Path::new(input).extension().and_then(|e| e.to_str()) == Some("vdb") {
                eprintln!(
                    "`.vdb` input detected. Use `vdb_convert from-vdb <file.vdb> <out.evol>` \
                     once native import lands; see --help for Phase 1 workarounds."
                );
                std::process::exit(2);
            }
            if input.ends_with(".evol") {
                eprintln!("input already appears to be .evol; use `vdb_convert info <path>`");
                std::process::exit(1);
            }
            eprintln!("unknown input `{input}` — expected .vdb (future) or a subcommand");
            print_usage();
            std::process::exit(1);
        }
    }
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
        "vdb_convert — Phase 1 bridge to Unity volume textures (.evol)\n\
         \n\
         Usage:\n\
           vdb_convert info <file.evol>\n\
           vdb_convert from-smoke-preset <out.evol>\n\
           vdb_convert from-vdb <file.vdb> <out.evol>   (stub — not yet implemented)\n\
         \n\
         Format: {format}\n\
         Unity: install integrations/unity/ElfentierFX.OpenVDB and import via\n\
                ElfentierFX → Import Volume / OpenVDB…",
        format = VOLUME_TEXTURE_FORMAT
    );
}
