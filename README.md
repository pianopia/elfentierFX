# elfentierFX

Unity-oriented procedural DCC for game-ready advanced looks — node-based modeling, volumes, fluids, and bake/export.

**FX** = effects (fluids, volumes, VFX).

## Stack

| Layer | Technology |
|-------|------------|
| UI shell | Tauri 2 + React / Vite (React Flow node editor) |
| 3D viewport | **Native Rust `wgpu`** offscreen renderer (Vulkan/Metal/DX12) → canvas preview |
| Viewport fallback | Three.js WebGPURenderer / WebGL when wgpu preview unavailable |
| Core | Rust crate (`elfentier_core`); C++/OpenVDB via FFI later |
| Platforms | macOS, Windows, Linux |

### Viewport architecture

Realtime mesh and smoke display target **native OS graphics** via `wgpu`, not the WebView as the performance path. The Tauri webview keeps React Flow and the prompt bar; cooked geometry is rendered in Rust (`apps/desktop/src-tauri/src/wgpu_viewport.rs`) and returned as RGBA for a canvas preview. Drag-to-orbit re-invokes the native renderer with an updated camera. Three.js remains as a temporary fallback.

## Repository layout

```
apps/desktop/          Tauri 2 desktop app (Rust host + React frontend)
crates/elfentier_core/ Shared procedural core (building, fluids, graph cook, export)
docs/agent-api.md      JSON command surface for agents
```

## Alpha 2 + Fluids Phase 1

Building → city workflow plus smoke/gas:

- **Smoke / gas (Phase 1)** — Eulerian solver in `elfentier_core` (`SmokeDomain` → `SmokeSource` → `SmokeSolver` → `SmokeRoot`); animated soft particle impostors from density; **煙 · Smoke puff** preset
- **Native wgpu preview** — instanced mesh + smoke particle billboards rendered offscreen; `render_native_viewport_command` for orbit/animation frames
- **Instanced cook** — `cook` returns mesh buffers + optional native preview image
- **Prompt bar** — local JP/EN interpreter (`商店街`, `煙`, `もっと煙`, `階数を5に`, …)
- **Agent API** — see [docs/agent-api.md](docs/agent-api.md)

OpenVDB I/O and FLIP liquids remain on the roadmap (Phase 2).

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
3. **Export glTF** / **Export smoke** as appropriate

## Workspace commands

```bash
cargo check --workspace
cargo test -p elfentier_core
cargo test -p elfentierfx-desktop
cd apps/desktop && npm run typecheck
```

## License

MIT — see [LICENSE](LICENSE).
