# elfentierFX

Unity-oriented procedural DCC for game-ready advanced looks — node-based modeling, volumes (OpenVDB), fluids, and bake/export.

**FX** = effects (fluids, volumes, VFX).

## Stack

| Layer | Technology |
|-------|------------|
| UI shell | Tauri 2 + React / Vite (React Flow–style node editor) |
| Core | Rust crate (`elfentier_core`); C++/OpenVDB via FFI later |
| Platforms | macOS, Windows, Linux |

## Repository layout

```
apps/desktop/          Tauri 2 desktop app (Rust host + React frontend)
crates/elfentier_core/ Shared procedural core (graph/mesh stubs, Alpha 0 API)
```

## Alpha 0 (current)

- Desktop app boots with a dark UI and empty node-canvas placeholder
- Tauri commands call into `elfentier_core` (`core_version`, `create_box_mesh`)
- CI runs `cargo check` and frontend typecheck on Ubuntu

**Alpha 1** (next): building → city procedural vertical slice. Fluids and OpenVDB remain on the roadmap.

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

# Frontend typecheck only
cd apps/desktop && npm run typecheck
```

## License

MIT — see [LICENSE](LICENSE).
