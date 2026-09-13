# elfentierFX

Unity-oriented procedural DCC for game-ready advanced looks — node-based modeling, volumes, fluids, and bake/export.

**FX** = effects (fluids, volumes, VFX).

## Stack

| Layer | Technology |
|-------|------------|
| UI shell | Tauri 2 + React / Vite (React Flow node editor) |
| 3D viewport (smoke) | **Native Rust `wgpu`** offscreen raymarch → canvas preview (Vulkan/Metal/DX12) |
| 3D viewport (city mesh) | Three.js WebGPURenderer / WebGL fallback (temporary mesh path) |
| Core | Rust crate (`elfentier_core`); C++/OpenVDB via FFI later |
| Platforms | macOS, Windows, Linux |

### Viewport architecture

Realtime smoke visualization targets **native OS graphics** via `wgpu`, not WebView WebGPU as the long-term path. The Tauri webview keeps React Flow + the prompt bar; cooked smoke density is raymarched in Rust (`apps/desktop/src-tauri/src/wgpu_viewport.rs`) and returned as an RGBA buffer displayed on a slim canvas. City/building meshes still use the Three.js instancing fallback until they migrate to the same native surface pattern.

## Repository layout

```
apps/desktop/          Tauri 2 desktop app (Rust host + React frontend)
crates/elfentier_core/ Shared procedural core (building, fluids, graph cook, export)
docs/agent-api.md      JSON command surface for agents
```

## Alpha 3 (current)

Fluids Phase 1 — smoke/gas on top of Alpha 2 city workflow:

- **Eulerian smoke solver** — density + velocity grid, emit/advect/diffuse/buoyancy/pressure projection (`crates/elfentier_core/src/smoke.rs`)
- **Smoke graph nodes** — `SmokeDomain`, `SmokeSource`, `SmokeSolver`, `SmokeRoot` + **Smoke plume** preset
- **Native wgpu preview** — offscreen volume raymarch in the Tauri host; CPU fallback when GPU unavailable
- **Prompt hooks** — `煙`, `smoke`, `smoke plume` load the smoke preset
- **Unity export stub** — raw f32 density volume (`elfentier_smoke_v1`) with header for Texture3D / flipbook import
- **City path preserved** — shop street / grid block presets, glTF export, Three mesh viewport unchanged

**Alpha 2** shipped the real-time mesh viewport, prompt interpreter, and agent API.

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

### Alpha 3 quick path

1. Launch the app — **Shop → Street** preset loads by default
2. Click **Smoke plume** (or prompt `煙` / `smoke`) to switch graphs
3. Click **Cook** — native wgpu raymarch preview appears in the viewport
4. Click **Export volume** — writes `/tmp/elfentier_smoke.raw` (Unity-oriented density dump)

City workflow (Alpha 2): adjust building params, **Cook**, **Export glTF** → `/tmp/elfentier_city.glb`.

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

# Core unit tests (building, placement, smoke, graph, export, prompt, viewport)
cargo test -p elfentier_core

# Frontend typecheck only
cd apps/desktop && npm run typecheck
```

## License

MIT — see [LICENSE](LICENSE).
