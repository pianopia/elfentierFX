# ElfentierFX Unity package

Import cooked elfentierFX export bundles into Unity 6.

## Install

1. Open Unity 6 (6000.x) and create or open a project.
2. Copy this folder into your project as `Packages/com.pianopia.elfentierfx`, **or** add it via **Window → Package Manager → + → Add package from disk…** and select `package.json`.
3. Wait for compilation (Runtime + Editor assemblies).

## Export from elfentierFX

1. Cook a city, smoke, or liquid graph in the desktop app.
2. Click **Export Bundle** (writes a directory with `manifest.json` + payloads).
3. Note the bundle folder path (default: system temp, e.g. `/tmp/elfentier_export_ocean_patch_…`).

## Import in Unity

1. **ElfentierFX → Import Export Bundle…**
2. Select the bundle folder (must contain `manifest.json`).
3. The importer spawns a root GameObject:
   - **City**: loads `city_mesh.glb` via `GltfUtility` stub path (copy GLB into `Assets/` manually if needed) or references the file path.
   - **Liquid**: creates `ElfentierLiquidPlayback` with parsed particle frames.
   - **Smoke**: logs atlas metadata (texture import stub).

## Runtime playback (liquid)

Attach `ElfentierLiquidPlayback` to a GameObject. Assign a material using **Particles/Standard Unlit** or URP equivalent. The component spawns child sphere instances per frame at 12 fps.

## Format reference

See [`docs/export-formats.md`](../../../docs/export-formats.md) in the main repository.

## OpenVDB / volume playback

Install the companion package **`integrations/unity/ElfentierFX.OpenVDB/`** for `volume_texture.evol` import, `ElfentierVolumePlayer`, and **ElfentierFX → Import Volume / OpenVDB…**.

Smoke bundles now include a `volume_texture.evol` payload (`elfentier_volume_texture_v1`). When the OpenVDB package is present, bundle import auto-spawns a volume player.

## Limitations (Phase 1)

- GLB import uses a minimal path copy; use Unity glTFast or similar for production mesh import.
- Native `.vdb` import requires the OpenVDB companion package converter path (see its README).
- No live bridge to a running elfentierFX session yet.
