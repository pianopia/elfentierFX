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

## Volume playback (3D density)

Smoke bundles include `volume_texture.evol` (`elfentier_volume_texture_v1`). This package imports bundle metadata only; for **3D volume import and scene playback**, install the separate product **[Unity Volume Importer](https://github.com/pianopia/UnityVolumeImporter)**:

- UPM: `com.louddin.unity-volume-importer`
- Git URL: `https://github.com/pianopia/UnityVolumeImporter.git?path=Packages/com.louddin.unity-volume-importer`

Unity Volume Importer reads legacy `.evol` from elfentierFX exports. See [`integrations/unity/ElfentierFX.OpenVDB/README.md`](../ElfentierFX.OpenVDB/README.md) for the migration note.

## Format reference

See [`docs/export-formats.md`](../../../docs/export-formats.md) in the main repository.

## Limitations (Phase 1)

- GLB import uses a minimal path copy; use Unity glTFast or similar for production mesh import.
- No live bridge to a running elfentierFX session yet.
