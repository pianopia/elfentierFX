# elfentierFX Agent API

JSON-serializable Tauri commands for external agents and the desktop UI. All commands accept and return plain JSON — no API keys required for the local interpreter path.

## Commands

| Command | Input | Output | Description |
|---------|-------|--------|-------------|
| `get_core_version` | — | `string` | Core library semver |
| `get_graph` | `{ graph }` | `Graph` | Echo / validate a graph document |
| `list_presets_command` | — | `PresetInfo[]` | Built-in starter graphs |
| `load_preset` | `{ preset_id }` | `Graph` | Load preset by id (`shop_street`, `grid_block`, `smoke_puff`) |
| `get_shop_street_preset` | — | `Graph` | Shop street starter graph |
| `get_smoke_puff_preset` | — | `Graph` | Smoke/gas puff starter graph |
| `set_params_command` | `{ request: { graph, node_id?, params } }` | `Graph` | Update `BuildingParams` on a node |
| `set_smoke_params_command` | `{ request: SetSmokeParamsRequest }` | `Graph` | Update smoke domain/source/solver nodes |
| `cook` | `{ graph }` | `CookWithMeshResult` | Cook graph + viewport buffers + native wgpu preview |
| `render_native_viewport_command` | `{ request: RenderNativeRequest }` | `NativePreviewImage` | Re-render mesh/smoke frame with camera (orbit/animation) |
| `cook_city_graph` | `{ graph }` | `CookResult` | Stats only (legacy) |
| `export_gltf` | `{ graph, path }` | `ExportResult` | Export merged city `.glb` |
| `export_smoke_density_command` | `{ request: { graph, path } }` | `SmokeExportResult` | Export smoke density atlas stub |
| `apply_prompt_command` | `{ request: { graph, prompt } }` | `ApplyPromptResult` | Local NL → graph edits |
| `explain_graph_command` | `{ graph }` | `string` | Template graph summary |

## Graph document

```json
{
  "name": "Shop Street",
  "nodes": [
    {
      "id": "building_params",
      "kind": "building_params",
      "label": "Shop Params",
      "building_params": {
        "floors": 2,
        "width": 6.0,
        "depth": 5.0,
        "window_density": 0.65,
        "seed": 42,
        "floor_height": 3.2
      }
    }
  ],
  "edges": [{ "from": "building_params", "to": "building_mesh" }]
}
```

### Node kinds

**City:** `building_params`, `building_mesh`, `place_along_path`, `fill_grid`, `merge_instances`, `city_root`

**Smoke (Phase 1):** `smoke_domain`, `smoke_source`, `smoke_solver`, `smoke_root`

Smoke preset chain:

```
smoke_domain → smoke_source → smoke_solver → smoke_root
```

### Smoke params (on nodes)

`smoke_domain`:

```json
{
  "resolution": 24,
  "bounds_min": { "x": -4, "y": 0, "z": -4 },
  "bounds_max": { "x": 4, "y": 8, "z": 4 },
  "seed": 7
}
```

`smoke_source`:

```json
{
  "position": { "x": 0, "y": 1.2, "z": 0 },
  "radius": 0.9,
  "emission_rate": 2.5,
  "temperature": 1.4,
  "upward_velocity": 2.8
}
```

`smoke_solver`:

```json
{
  "steps": 48,
  "frame_stride": 4,
  "dissipation": 0.985,
  "buoyancy": 1.6,
  "diffusion": 0.12,
  "pressure_iterations": 18,
  "ground_collision": true,
  "max_particles_per_frame": 1800
}
```

## Cook viewport mesh (`cook`)

Returns instanced geometry for city graphs, or animated smoke impostor particles for smoke graphs:

```json
{
  "stats": {
    "vertex_count": 0,
    "instance_count": 0,
    "graph_name": "Smoke Puff",
    "smoke_resolution": [24, 24, 24],
    "smoke_max_density": 0.42,
    "smoke_steps": 48,
    "smoke_frame_count": 13,
    "smoke_particle_count": 842
  },
  "mesh": {
    "positions": [],
    "indices": [],
    "instance_matrices": [1, 0, 0, 0, "..."],
    "smoke": {
      "frame_count": 13,
      "fps": 12,
      "bounds_min": [-4, 0, -4],
      "bounds_max": [4, 8, 4],
      "frames": [
        {
          "particle_count": 842,
          "positions": [0.1, 1.2, 0.0, "..."],
          "sizes": [0.35, "..."],
          "opacities": [0.5, "..."]
        }
      ],
      "stats": { "resolution": [24, 24, 24], "max_density": 0.42, "step_count": 48 }
    }
  }
}
```

- City graphs: `positions` / `indices` / `instance_matrices` as before.
- Smoke graphs: `smoke.frames[]` holds soft particle impostors; viewport autoplays at `smoke.fps`.

Export (`export_gltf`) writes a **merged** mesh for city graphs only.

## Smoke density export (`export_smoke_density_command`)

Unity-oriented stub: writes a small header plus raw `f32` density atlas bytes.

```json
{
  "path": "/tmp/elfentier_smoke_density.raw",
  "frame_count": 13,
  "byte_len": 4096,
  "format": "elfentier_smoke_atlas_v1"
}
```

Format: text header (`# elfentier smoke density atlas v1`) followed by little-endian `f32` data (XY slices, frame-major). Suitable for flipbook or 3D texture import in Unity.

## Prompt interpreter (`apply_prompt_command`)

Local rule-based mapping (JP + EN). Designed so a cloud LLM can later emit the same `GraphEdit` list.

Example phrases:

| Phrase | Effect |
|--------|--------|
| `商店街` / `shop street` | Load shop street preset |
| `煙` / `smoke` / `smoke puff` | Load smoke puff preset |
| `もっと煙` / `more smoke` | Increase emission + simulation steps |
| `階数を5に` / `5 floors` | Set floor count |
| `seed変え` / `change seed` | Randomize seed |
| `もっと窓` / `more windows` | Increase window density |
| `グリッド配置` / `grid placement` | Switch to grid placement node |

Response shape:

```json
{
  "graph": { "...": "updated Graph" },
  "intents": [{ "load_preset": { "preset_id": "smoke_puff" } }],
  "edits": [{ "load_preset": { "preset_id": "smoke_puff" } }],
  "summary": "Loaded preset smoke_puff"
}
```

## Workflow

1. **Graph is source of truth** — prompts only mutate the graph JSON.
2. **Cook is deterministic** — same graph + seed always yields the same simulation.
3. Agents should call `apply_prompt_command` → `cook` → optionally `export_gltf` or `export_smoke_density_command`.

## Future

- MCP server wrapping these commands (not implemented in Alpha 2).
- OpenVDB volume I/O and FLIP liquids (Phase 2).
- Optional cloud LLM backend behind `apply_prompt` (stub only; no keys required today).
