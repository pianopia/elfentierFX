# elfentierFX

Unity-oriented procedural DCC for game-ready advanced looks — node-based modeling, volumes (OpenVDB), fluids, and bake/export.

**FX** = effects (fluids, volumes, VFX).

## Stack

| Layer | Technology |
|-------|------------|
| UI shell | Tauri 2 + React / Vite (React Flow node editor) |
| Core | Rust crate (`elfentier_core`); C++/OpenVDB via FFI later |
| Platforms | macOS, Windows, Linux |

## Repository layout

```
apps/desktop/          Tauri 2 desktop app (Rust host + React frontend)
crates/elfentier_core/ Shared procedural core (building, placement, graph cook, export)
```

## Alpha 1 (current)

End-to-end **building → city** vertical slice:

- **Building rule** — `BuildingParams` (floors, width, depth, window density, seed) generates a mesh with floor volume, facade window recesses, and roof cap
- **Placement** — `PlaceAlongPath` and `FillGrid` produce instance transforms from a building recipe
- **Graph nodes** — `BuildingParams`, `BuildingMesh`, `PlaceAlongPath`, `FillGrid`, `MergeInstances`, `CityRoot` wired in `elfentier_core`
- **UI** — React Flow canvas with shop-street starter graph; tweak floors/seed/window density; **Cook** shows verts/tris/instance count; **Export glTF** writes a combined `.glb`
- **Preset** — one-click **Shop → Street** loads a connected starter graph

**Alpha 0** shipped the Tauri shell, empty canvas placeholder, and unit-box stub API.

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

### Alpha 1 quick path

1. Launch the app — the **Shop → Street** preset graph loads automatically
2. Adjust **Floors**, **Seed**, or **Window density** on the Building Params node
3. Click **Cook** — footer shows instance / vertex / triangle counts
4. Click **Export glTF** — writes `/tmp/elfentier_city.glb` (combined instanced geometry)

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

# Core unit tests (building mesh, placement math, graph cook, glTF export)
cargo test -p elfentier_core

# Frontend typecheck only
cd apps/desktop && npm run typecheck
```

## License

MIT — see [LICENSE](LICENSE).
