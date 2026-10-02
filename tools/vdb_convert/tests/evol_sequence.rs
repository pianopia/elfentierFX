use elfentier_core::openvdb_io::read_openvdb_density;
use std::io::Write;
use std::path::Path;

fn fixture(path: &Path) {
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(b"EFVT").unwrap();
    for n in [1_u32, 2, 3, 4, 2, 1] { file.write_all(&n.to_le_bytes()).unwrap(); }
    for n in [1_f32, 2.0, 3.0, 5.0, 8.0, 11.0] { file.write_all(&n.to_le_bytes()).unwrap(); }
    for frame in 0..2 { for n in 0..24 { file.write_all(&(1.0 + n as f32 + frame as f32 * 100.0).to_le_bytes()).unwrap(); } }
}
#[test]
fn evol_sequence_preserves_values_and_ue_axes_bounds() {
    let root = std::env::temp_dir().join(format!("elfentier_evol_seq_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("input.evol"); fixture(&input);
    let paths = vdb_convert::convert_evol(&input, &root.join("density.vdb"), true, true).unwrap();
    assert_eq!(paths.len(), 2);
    let grid = read_openvdb_density(paths[1].to_str().unwrap(), None).unwrap();
    assert_eq!(grid.resolution, [2, 4, 3]);
    assert_eq!(grid.bounds_min, [100.0, 300.0, 200.0]);
    assert_eq!(grid.bounds_max, [500.0, 1100.0, 800.0]);
    // Source x=1,y=2,z=3 becomes UE x=1,y=3,z=2.
    assert_eq!(grid.density[1 + 2 * (3 + 4 * 2)], 124.0);
    assert_eq!(grid.density[0], 101.0);
    assert!(vdb_convert::convert_evol(&input, &root.join("density.vdb"), true, true).is_err());
    for path in paths { std::fs::remove_file(path).unwrap(); }
    std::fs::remove_file(input).unwrap(); std::fs::remove_dir(root).unwrap();
}
#[test]
fn truncated_evol_is_rejected_before_output() {
    let root = std::env::temp_dir().join(format!("elfentier_evol_bad_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("input.evol"); fixture(&input);
    std::fs::OpenOptions::new().write(true).open(&input).unwrap().set_len(55).unwrap();
    let output = root.join("out.vdb");
    assert!(vdb_convert::convert_evol(&input, &output, false, false).is_err());
    assert!(!output.exists());
    std::fs::remove_file(input).unwrap(); std::fs::remove_dir(root).unwrap();
}
