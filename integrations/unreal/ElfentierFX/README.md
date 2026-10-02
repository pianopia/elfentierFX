# ElfentierFX Unreal plugin (source skeleton)

Phase 1 plugin source for importing elfentierFX export bundles into Unreal Engine 5.x. No prebuilt binaries are shipped — compile from source inside your UE project.

## Install

1. Copy `integrations/unreal/ElfentierFX/` into your project's `Plugins/ElfentierFX/` folder.
2. Regenerate project files and build the Editor target.
3. Enable **ElfentierFX** under **Edit → Plugins**.

## Export from elfentierFX

Cook a graph in the desktop app, then click **Export Bundle**. The bundle directory contains:

- `manifest.json` — `elfentier_export_manifest_v1`
- `graph.json` — graph snapshot
- Payload file(s) depending on graph mode

## Import paths

### Static mesh (city / GLB)

1. Use Unreal's **Interchange glTF importer** (GLTFExporter is an export plugin).
2. Point at `city_mesh.glb` inside the bundle directory.
3. Source data is **meters, Y-up**. Standard Interchange converts to Unreal's **centimeters, Z-up**; do not apply a second manual rotation/scale.

### External Three.js → UE native assets

See [Three.js converter](../../../tools/three_to_unreal/README.md).
`Content/Python/import_three_bundle.py` imports the separate
`elfentier_three_unreal_v1` manifest, creates native Custom HLSL/water materials,
assigns them to StaticMesh slots, and imports baked Alembic/VDB payloads.
It can also be loaded directly without compiling this plugin's C++ skeleton.
Enable Python Editor Script Plugin, Editor Scripting Utilities, Interchange,
and Alembic Importer. VDB frames import as individual static SVTs; animated SVT
sequence import and HeterogeneousVolume material/actor setup are manual.

### Liquid particle cache

The C++ loader `FElfentierLiquidCacheLoader` parses `elfentier_liquid_cache_v1`:

```cpp
TArray<FElfentierLiquidFrame> Frames;
FElfentierLiquidCacheLoader::LoadFromFile(TEXT("/path/to/liquid_cache.raw"), Frames);
```

Use frames to drive:

- **Niagara** — spawn particles per frame from CPU data (Phase 1: manual Niagara setup; see `Content/Python/import_elfentier_bundle.py`).
- **Simple Actor** — sample `AElfentierLiquidPreviewActor` (stub) to visualize spheres per frame.

### Python utility (Editor)

Run from Unreal's Python console after adjusting `BUNDLE_DIR`:

```python
exec(open("Plugins/ElfentierFX/Content/Python/import_elfentier_bundle.py").read())
```

## Format reference

[`docs/export-formats.md`](../../../docs/export-formats.md)

## Limitations (Phase 1)

- No auto-generated Niagara graphs
- No live TCP bridge (stub comments in module header)
- GLB import uses standard UE glTF path, not bundled here

## Future

- Editor Utility Widget for folder picker
- Niagara module emitting from `elfentier_liquid_cache_v1` directly
