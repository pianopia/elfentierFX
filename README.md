# elfentierFX

Unity-oriented procedural DCC for game-ready advanced looks — node-based modeling, volumes, fluids, and bake/export.

**FX** = effects (fluids, volumes, VFX).

## Stack

| Layer | Technology |
|-------|------------|
| UI shell | Tauri 2 + React / Vite (React Flow node editor) |
| 3D viewport | **Native Rust `wgpu` only** — offscreen renderer (Vulkan/Metal/DX12) → canvas preview |
| Core | Rust crate (`elfentier_core`); OpenVDB fog I/O via pure-Rust writer + `vdb-rs` reader |
| Platforms | macOS, Windows, Linux |

### Viewport architecture

Realtime mesh and smoke display use **native OS graphics only** via `wgpu`, not the WebView as the performance path. The Tauri webview keeps React Flow and the prompt bar; cooked geometry is rendered in Rust (`apps/desktop/src-tauri/src/wgpu_viewport.rs`) and returned as RGBA for a canvas preview. Drag-to-orbit re-invokes the native renderer with an updated camera. If wgpu init or render fails, the viewport shows an explicit error panel (no WebGL/Three.js fallback).

## Repository layout

```
apps/desktop/          Tauri 2 desktop app (Rust host + React frontend)
crates/elfentier_core/ Shared procedural core (building, fluids, graph cook, export)
docs/agent-api.md      JSON command surface for agents
docs/export-formats.md Cook export bundle + payload format reference
integrations/          Unity, Unreal, Blender import packages
tools/vdb_convert/     CLI for .evol interchange and OpenVDB fog .vdb I/O
```

## Alpha 2 + Fluids Phase 1

Building → city workflow plus smoke/gas:

- **Smoke / gas (Phase 1)** — Eulerian solver in `elfentier_core` (`SmokeDomain` → `SmokeSource` → `SmokeSolver` → `SmokeRoot`); animated soft particle impostors from density; **煙 · Smoke puff** preset
- **Native wgpu preview** — instanced mesh + smoke particle billboards rendered offscreen; `render_native_viewport_command` for orbit/animation frames
- **Instanced cook** — `cook` returns mesh buffers + optional native preview image
- **Prompt bar** — local JP/EN interpreter (`商店街`, `煙`, `もっと煙`, `階数を5に`, …)
- **FLIP liquids (Phase 1)** — ocean / waterfall / flood presets; particle cache export
- **Export bundles** — manifest + payloads for Unity, Unreal, Blender (`Export Bundle` in UI)
- **Agent API** — see [docs/agent-api.md](docs/agent-api.md) and [docs/export-formats.md](docs/export-formats.md)

**Alpha 2 (OpenVDB I/O spike):** smoke graphs export an optional `smoke_density.vdb` fog FloatGrid (last frame) alongside `.evol` / atlas payloads. Reading uses `vdb-rs`; writing is a pure-Rust minimal encoder (uncompressed active-mask). Open `.vdb` in Houdini, Blender, or other OpenVDB tools. Unity users install **[Unity Volume Importer](https://github.com/pianopia/UnityVolumeImporter)** (`com.louddin.unity-volume-importer`) for legacy `.evol` in-editor import — native `.vdb` there remains on that product's roadmap.

OpenVDB is a trademark of LF Projects, LLC.

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20+
- Platform deps for [Tauri 2](https://v2.tauri.app/start/prerequisites/)

## Run locally

```bash
cd apps/desktop
npm install
cargo tauri dev --manifest-path src-tauri/Cargo.toml
```

### Quick path

1. **Shop → Street** or **煙 · Smoke puff** preset
2. **Cook** — native wgpu preview (drag canvas to orbit; smoke animates at sim fps)
3. **Export glTF** / **Export smoke** / **Export liquid** / **Export Bundle** as appropriate

## Workspace commands

```bash
cargo check --workspace
cargo test -p elfentier_core
cargo test -p vdb_convert
cargo test -p elfentierfx-desktop
cd apps/desktop && npm run typecheck
```

## License

MIT — see [LICENSE](LICENSE).
