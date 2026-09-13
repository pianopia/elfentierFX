# elfentierFX

Unity-oriented procedural DCC for game-ready advanced looks — node-based modeling, volumes (OpenVDB), fluids, and bake/export.

**FX** = effects (fluids, volumes, VFX).

## Stack

| Layer | Technology |
|-------|------------|
| UI shell | Tauri 2 + React / Vite (React Flow node editor) |
| 3D viewport | Three.js WebGPURenderer (WebGL fallback) + GPU instancing |
| Core | Rust crate (`elfentier_core`); C++/OpenVDB via FFI later |
| Platforms | macOS, Windows, Linux |

## Repository layout

```
apps/desktop/          Tauri 2 desktop app (Rust host + React frontend)
crates/elfentier_core/ Shared procedural core (building, placement, graph cook, export)
docs/agent-api.md      JSON command surface for agents
```

## Alpha 2 (current)

Extends Alpha 1 with a real-time viewport and AI-era workflow foundations:

- **3D viewport** — resizable split: node graph + WebGPU-first preview with orbit/pan/zoom, studio lighting, ground grid, FPS/tri/draw-call chrome
- **Instanced cook** — `cook` returns base mesh buffers + per-instance 4×4 matrices (efficient transfer; export still merges for glTF)
- **Prompt bar** — local JP/EN interpreter maps phrases (`商店街`, `階数を5に`, `もっと窓`, `グリッド配置`) to graph edits without an API key
- **Agent API** — `get_graph`, `set_params`, `cook`, `export_gltf`, `list_presets`, `apply_prompt`, `explain_graph` (see [docs/agent-api.md](docs/agent-api.md))
- **Explain graph** — template summary of the current graph in the UI

**Alpha 1** shipped building → city graph cook, React Flow editor, and glTF export.

Fluids and OpenVDB remain on the roadmap.

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20+
- Platform deps for [Tauri 2](https://v2.tauri.app/start/prerequisites/) (WebKitGTK on Linux, Xcode CLT on macOS, MSVC + WebView2 on Windows)

## Run locally

```bash
# Install frontend dependencies
cd apps/desktop
npm install

# Dev mode (Vite + Tauri window)
cargo tauri dev
```

From the repo root you can also run:

```bash
cargo tauri dev --manifest-path apps/desktop/src-tauri/Cargo.toml
```

### Alpha 2 quick path

1. Launch the app — **Shop → Street** preset loads automatically
2. Adjust **Floors**, **Seed**, or **Window density** on the Building Params node, or type a prompt (e.g. `階数を5に`)
3. Click **Cook** — 3D viewport shows instanced buildings; footer shows stats
4. Click **Export glTF** — writes `/tmp/elfentier_city.glb` (merged geometry)

## Build

```bash
cd apps/desktop
npm run build
cargo tauri build --manifest-path src-tauri/Cargo.toml
```

## Workspace commands

```bash
# Check all Rust crates
cargo check --workspace

# Core unit tests (building mesh, placement math, graph cook, glTF export, prompt, viewport)
cargo test -p elfentier_core

# Frontend typecheck only
cd apps/desktop && npm run typecheck
```

## License

MIT — see [LICENSE](LICENSE).
