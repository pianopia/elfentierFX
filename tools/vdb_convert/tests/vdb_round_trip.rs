use elfentier_core::graph::Graph;
use elfentier_core::openvdb_io::{export_openvdb_fog, read_openvdb_info};
use std::process::Command;

#[test]
fn to_vdb_from_preset_cli_writes_readable_file() {
    let out = std::env::temp_dir().join("elfentier_vdb_convert_test.vdb");
    let out_str = out.to_string_lossy().to_string();
    let _ = std::fs::remove_file(&out);

    let status = Command::new(env!("CARGO_BIN_EXE_vdb_convert"))
        .args(["to-vdb", &out_str, "--from-preset"])
        .status()
        .expect("run vdb_convert");
    assert!(status.success());
    assert!(out.exists());

    let info = read_openvdb_info(&out_str).expect("read info");
    assert_eq!(info.grid_name, "density");
    assert!(info.voxel_count > 0);

    let graph = Graph::smoke_puff_preset();
    let volume = elfentier_core::graph::evaluate_smoke_volume(&graph).expect("cook");
    let direct = export_openvdb_fog(&volume, &out_str).expect("direct export");
    assert!(direct.byte_len > 0);

    let _ = std::fs::remove_file(&out);
}
