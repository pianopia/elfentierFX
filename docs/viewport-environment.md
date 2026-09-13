# Viewport HDR / environment

The native wgpu viewport can render smoke and meshes against an **equirectangular HDR environment** instead of the legacy flat gray clear color. This is the Phase A lookdev path: readable background + simple image-based lighting for volume raymarch (not full path-traced IBL).

## UI (desktop)

In the **3D View** panel:

| Control | Effect |
|---------|--------|
| **Env** preset | `Studio soft` (default), `Studio contrast`, `Flat gray`, or `Custom HDR` |
| **Intensity** | Exposure multiplier on environment radiance |
| **Yaw** | Rotate the environment around world Y |
| **Diffuse blur** | Wider kernel for diffuse environment samples (volume ambient) |
| **Pick HDR** | (Custom only) Native file dialog for `.hdr`, `.exr`, `.png`, `.jpg` |

Settings persist in `localStorage` under `elfentier-viewport-environment`.

## Built-in presets (offline)

- **Studio soft** — warm floor gradient + soft key; default for smoke film lookdev
- **Studio contrast** — darker sides, brighter overhead strip
- **Flat gray** — legacy clear color (environment disabled)

No external file is required for the studio presets.

## Custom files

Custom maps must be **equirectangular (lat-long)**. Supported extensions:

- `.hdr` — Radiance RGBE
- `.exr` — OpenEXR (first RGBA layer)
- `.png` / `.jpg` — LDR (treated as linear-ish for preview)

## Rendering path

1. **Background pass** — full-screen triangle samples the env map by view ray (Reinhard tonemap to LDR `RGBA8UnormSrgb`).
2. **Volume raymarch** — diffuse env sample along the ray + key sample along light direction modulate in-scatter; smoke tints pick up env color.
3. **Cook / orbit** — `cook` and `render_native_viewport_command` accept optional `environment` in the request (defaults to studio soft).

## Film lookdev note

When judging smoke **density** and silhouette, use a studio HDR background. Flat gray makes thin plumes hard to read; environment contrast separates smoke from backdrop similarly to on-set reference.

## API

Rust types live in `elfentier_core::environment::ViewportEnvironment`. The wgpu renderer entry point:

```rust
render_native_viewport(mesh, frame, width, height, camera, &environment)
```

Tauri: `get_default_viewport_environment`, `pick_hdr_file_command`, and `environment` on `RenderNativeRequest`.
