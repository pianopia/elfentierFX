# ElfentierFX OpenVDB (Unity)

Phase 1 Unity package for **volume import and scene playback** from elfentierFX exports. Unity does not ship a first-class OpenVDB pipeline; this package fills that gap with a converter-friendly `elfentier_volume_texture_v1` path while keeping an `IVdbImporter` hook for native `.vdb` in a later phase.

Unreal Engine already exposes stronger native VDB tooling; this package targets Unity 6 projects that need a practical volume handoff today.

## Supported Unity version

- **Unity 6** (6000.x), HDRP and URP projects (shader is Built-in compatible unlit raymarch stub).

## Install

1. Install the base bundle package first: `integrations/unity/ElfentierFX/`.
2. Add this folder via **Window → Package Manager → + → Add package from disk…** and select `package.json`.
3. Or copy both packages under `Packages/com.pianopia.elfentierfx` and `Packages/com.pianopia.elfentierfx.openvdb`.

## Import paths (Phase 1)

| Source | Menu | Output |
|--------|------|--------|
| `.evol` volume texture (`elfentier_volume_texture_v1`) | **ElfentierFX → Import Volume / OpenVDB…** | `Texture3D` asset + `ElfentierVolumePlayer` in scene |
| elfentierFX export bundle (`volume_texture.evol` payload) | **ElfentierFX → Import Export Bundle…** (base package) | Auto-wires volume player when payload present |
| `.vdb` OpenVDB grid | Same menu (stub) | Logs Phase 1 guidance; run `tools/vdb_convert` or desktop export |

### Desktop / CLI conversion

From the repository root:

```bash
cargo run -p vdb_convert -- from-smoke-preset /tmp/smoke.evol
cargo run -p vdb_convert -- info /tmp/smoke.evol
```

Cook → **Export Bundle** in the desktop app writes `volume_texture.evol` beside `manifest.json` for smoke graphs.

## Runtime

Attach or spawn **`ElfentierVolumePlayer`** on a GameObject. It raymarches the assigned `Texture3D` (or animates multiple frames) using `ElfentierFX/VolumeRaymarch` material.

Fields:

- `volumeTexture` — imported 3D density
- `boundsMin` / `boundsMax` — world-space AABB (meters, Y-up)
- `playbackFps` — defaults to 12 (matches elfentierFX export)
- `densityScale`, `stepSize`, `maxSteps` — raymarch tuning

## Shader / render pipelines

`Runtime/Shaders/ElfentierVolumeRaymarch.shader` is a **Built-in unlit raymarch** stub. For HDRP/URP production:

- Duplicate the shader into your SRP shader library, or
- Sample `_VolumeTex` in a Shader Graph **Custom Function** with the same bounds/density uniforms.

## Architecture

- `IVdbImporter` — pluggable import back end (`ElfentierVolumeTextureImporter` today, `NativeVdbImporter` stub for `.vdb`).
- `ElfentierVolumeTextureReader` — parses `.evol` binary payloads.
- Editor scripted importer creates `Texture3D` assets under `Assets/ElfentierFX/Volumes/`.

## Known limits (Phase 1)

- No native OpenVDB/NanoVDB decode in-editor (converter + `.evol` only).
- Single-channel density; no velocity/temperature channels yet.
- Raymarch shader is a preview-quality stub, not a production fog/volumetric lighting solution.
- Texture3D size is limited by Unity platform caps (typically 2048³ max; elfentierFX defaults are much smaller).

## Format reference

See [`docs/export-formats.md`](../../../docs/export-formats.md) in the main repository.
