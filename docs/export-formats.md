# elfentierFX export formats

Cooked results leave elfentierFX as standalone files or as a **bundle directory** with a unified manifest. All spatial data uses **meters**, **Y-up**, and fluid playback defaults to **12 fps** (matching the desktop viewport preview).

## Bundle layout

```
my_export/
  manifest.json          # elfentier_export_manifest_v1
  graph.json             # source graph snapshot (elfentier_graph_v1)
  city_mesh.glb          # city graphs only
  smoke_density.raw      # smoke graphs only (XY atlas)
  volume_texture.evol    # smoke graphs only (3D Texture3D path)
  smoke_density.vdb      # smoke graphs only (OpenVDB fog FloatGrid, last frame)
  liquid_cache.raw       # liquid graphs only
```

Export from the desktop app via **Export Bundle**, or call the Tauri command `export_cook_bundle_command_handler`.

## Manifest (`elfentier_export_manifest_v1`)

```json
{
  "format": "elfentier_export_manifest_v1",
  "version": 1,
  "core_version": "0.1.0-alpha.2",
  "created_at": "1726217280",
  "graph_name": "Ocean Patch",
  "graph_mode": "liquid",
  "units": "meters",
  "up_axis": "Y",
  "frame_rate": 12.0,
  "payloads": [
    {
      "format": "elfentier_graph_v1",
      "path": "graph.json",
      "byte_len": 2048
    },
    {
      "format": "elfentier_liquid_cache_v1",
      "path": "liquid_cache.raw",
      "frame_count": 20,
      "byte_len": 384000,
      "bounds_min": [-6.0, 0.0, -6.0],
      "bounds_max": [6.0, 3.0, 6.0],
      "resolution": [20, 20, 20]
    }
  ]
}
```

| Field | Meaning |
|-------|---------|
| `graph_mode` | `city`, `smoke`, or `liquid` |
| `units` | Always `meters` |
| `up_axis` | Always `Y` |
| `frame_rate` | Playback fps for fluid payloads |
| `payloads[].path` | Relative path inside the bundle directory |

## glTF / GLB mesh (`gltf_glb`)

- Binary glTF 2.0 with merged triangle mesh
- City graphs only (`city_mesh.glb`)
- Importable in Unity, Unreal, Blender, and any glTF 2.0 viewer

## Volume texture (`elfentier_volume_texture_v1`)

Binary little-endian payload (`.evol` extension) for Unity `Texture3D` import and GPU raymarch preview.

| Offset | Size | Field |
|--------|------|-------|
| 0 | 4 | magic `EFVT` |
| 4 | 4 | version `u32` (=1) |
| 8 | 4 | `nx` |
| 12 | 4 | `ny` |
| 16 | 4 | `nz` |
| 20 | 4 | `frame_count` |
| 24 | 4 | `channels` (=1 density) |
| 28 | 12 | `bounds_min` xyz `f32` |
| 40 | 12 | `bounds_max` xyz `f32` |
| 52 | … | `f32` samples, frame-major, x-fastest indexing |

- Smoke graphs only (`volume_texture.evol` in bundles)
- Engine interchange: convert desktop exports or run `tools/vdb_convert`
- Unity: install **[Unity Volume Importer](https://github.com/pianopia/UnityVolumeImporter)** (`com.louddin.unity-volume-importer`) — reads legacy `.evol` from elfentierFX bundles

## Smoke density atlas (`elfentier_smoke_atlas_v1`)

Text header followed by little-endian `f32` density samples:

```
# elfentier smoke density atlas v1
# frames=13 res=48x48
# data=f32 little-endian row-major XY per slice, frame-major
```

- Smoke graphs only (`smoke_density.raw`)
- XY slices max-projected through Z, stacked frame-major; suitable for flipbook materials
- Prefer `volume_texture.evol` for full 3D density; import in Unity via **Unity Volume Importer**

## Liquid particle cache (`elfentier_liquid_cache_v1`)

Text header followed by little-endian `f32` tuples per particle:

```
# elfentier liquid particle cache v1
# frames=20 particles_per_frame=1200
# layout=frame-major [x,y,z,radius] f32 little-endian
```

- Liquid graphs only (`liquid_cache.raw`)
- Positions in meters, Y-up
- Import as point cache or instanced spheres in Unity, Unreal Niagara, or Blender

## Graph snapshot (`elfentier_graph_v1`)

JSON graph document (`graph.json`) included in every bundle for reproducibility and DCC-side metadata.

## Engine / DCC integrations

| Target | Path | Import entry point |
|--------|------|-------------------|
| Unity 6 (bundle) | `integrations/unity/ElfentierFX/` | **ElfentierFX → Import Export Bundle…** |
| Unity 6 (volume) | [Unity Volume Importer](https://github.com/pianopia/UnityVolumeImporter) (`com.louddin.unity-volume-importer`) | Import `.evol` / volume assets (separate product) |
| Unreal Engine | `integrations/unreal/ElfentierFX/` | Plugin skeleton + README import notes |
| Blender 4.x | `integrations/blender/elfentier_fx/` | **Import ElfentierFX Bundle** operator |

## OpenVDB fog volume (`openvdb_fog_floatgrid_v1`)

Standard OpenVDB `.vdb` archive containing a `Tree_float_5_4_3` FloatGrid named `density` with class **fog volume**.

| Property | Value |
|----------|-------|
| Extension | `.vdb` |
| Grid name | `density` |
| Value type | `float` (scalar density) |
| Compression | Active-mask, uncompressed (readable by `vdb-rs` and standard OpenVDB tools) |
| Frame | Last smoke density frame in the bundle |
| Units | Meters via `ScaleTranslateMap` transform |

- Smoke graphs only (`smoke_density.vdb` in bundles)
- Readable by `vdb-rs` and other standard OpenVDB-compatible tools
- Unity: native `.vdb` is not imported by the in-repo ElfentierFX bundle plugin; use external tools or emit `.evol` for **[Unity Volume Importer](https://github.com/pianopia/UnityVolumeImporter)** (`com.louddin.unity-volume-importer`)

OpenVDB is a trademark of LF Projects, LLC.

## CLI converter (`tools/vdb_convert`)

```bash
cargo run -p vdb_convert -- from-smoke-preset /tmp/smoke.evol
cargo run -p vdb_convert -- to-vdb /tmp/smoke.vdb --from-preset
cargo run -p vdb_convert -- info /tmp/smoke.vdb
cargo run -p vdb_convert -- from-vdb /tmp/smoke.vdb /tmp/smoke.evol
```

## Future (out of scope for Alpha 2 spike)

- Multi-frame `.vdb` sequences (one grid per frame in a single archive)
- Liquid → VDB (level-set / fog from FLIP particles)
- Live TCP bridge to running editors (interface stub comments only)
- Full Niagara graph authoring
- USD pipeline
