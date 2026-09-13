# ElfentierDCC

Procedural DCC aimed at **Unity / game-ready advanced looks** — node-based modeling, volumes (OpenVDB), fluids, and bake/export — with a much lower learning curve than traditional FX tools.

> Working name / product vision. Not affiliated with SideFX Houdini.

## Stack (decided)

- **UI shell:** Tauri 2 + Web (React Flow–style node editor)
- **Core:** shared native (C++ / Rust + C++ FFI for OpenVDB & simulation)
- **Targets:** macOS, Windows, Linux
- **Distribution (planned):** Apple Developer, Microsoft Store, Linux direct; Booth early access / support

## Status

Design / scaffolding. Building → city procedural flow is the planned **Alpha 1** vertical slice; fluids & OpenVDB follow on the roadmap.

## Related

Existing personal repo `pianopia/Elfentier` already occupied the bare name (GitHub treats `elfentier` / `Elfentier` as the same). This repo uses **ElfentierDCC** to disambiguate.

## License

TBD.
