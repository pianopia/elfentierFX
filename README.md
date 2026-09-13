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

Extends Alpha 1 with a real-time viewport, smoke/gas fluids (Phase 1), and AI-era workflow foundations:

- **3D viewport** — resizable split: node graph + WebGPU-first preview with orbit/pan/zoom, studio lighting, ground grid, FPS/tri/draw-call chrome
- **Smoke / gas (Phase 1)** — Eulerian solver in `elfentier_core` (`SmokeDomain` → `SmokeSource` → `SmokeSolver` → `SmokeRoot`); viewport shows animated soft particle impostors; **煙 · Smoke puff** preset and prompt mapping (`煙`, `smoke`, `もっと煙`)
- **Instanced cook** — `cook` returns base mesh buffers + per-instance 4×4 matrices (efficient transfer; export still merges for glTF)
- **Prompt bar** — local JP/EN interpreter maps phrases (`商店街`, `煙`, `階数を5に`, `もっと窓`, `グリッド配置`) to graph edits without an API key
- **Agent API** — `get_graph`, `set_params`, `set_smoke_params`, `cook`, `export_gltf`, `export_smoke_density`, `list_presets`, `apply_prompt`, `explain_graph` (see [docs/agent-api.md](docs/agent-api.md))
- **Explain graph** — template summary of the current graph in the UI

**Alpha 1** shipped building → city graph cook, React Flow editor, and glTF export.

OpenVDB I/O and FLIP liquids remain on the roadmap (Phase 2).

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

**Shop street**

1. Launch the app — **Shop → Street** preset loads automatically
2. Adjust **Floors**, **Seed**, or **Window density** on the Building Params node, or type a prompt (e.g. `階数を5に`)
3. Click **Cook** — 3D viewport shows instanced buildings; footer shows stats
4. Click **Export glTF** — writes `/tmp/elfentier_city.glb` (merged geometry)

**Smoke puff**

1. Click **煙 · Smoke puff** (or prompt `煙` / `smoke`)
2. Click **Cook** — viewport plays animated smoke particles inside the domain bounds
3. Prompt `もっと煙` to increase emission and step count, then cook again
4. Click **Export smoke** — writes `/tmp/elfentier_smoke_density.raw` (density atlas stub for Unity)

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
