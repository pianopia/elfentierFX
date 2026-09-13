# elfentierFX Agent API

JSON-serializable Tauri commands for external agents and the desktop UI. All commands accept and return plain JSON — no API keys required for the local interpreter path.

## Commands

| Command | Input | Output | Description |
|---------|-------|--------|-------------|
| `get_core_version` | — | `string` | Core library semver |
| `get_graph` | `{ graph }` | `Graph` | Echo / validate a graph document |
| `list_presets_command` | — | `PresetInfo[]` | Built-in starter graphs |
| `load_preset` | `{ preset_id }` | `Graph` | Load preset by id (`shop_street`, `grid_block`, `smoke_plume`) |
| `get_shop_street_preset` | — | `Graph` | Shop street starter graph |
| `get_smoke_plume_preset` | — | `Graph` | Smoke plume starter graph |
| `set_params_command` | `{ request: { graph, node_id?, params } }` | `Graph` | Update `BuildingParams` on a node |
| `cook` | `{ graph }` | `CookWithViewportResult` | Cook graph + viewport payload + optional wgpu smoke preview |
| `cook_city_graph` | `{ graph }` | `CookResult` | Stats only (legacy) |
| `export_gltf` | `{ graph, path }` | `ExportResult` | Export merged `.glb` (city graphs) |
| `export_smoke_volume` | `{ graph, path }` | `SmokeVolumeExport` | Raw f32 density volume (smoke graphs) |
| `apply_prompt_command` | `{ request: { graph, prompt } }` | `ApplyPromptResult` | Local NL → graph edits |
| `explain_graph_command` | `{ graph }` | `string` | Template graph summary |

## Graph document

City and smoke graphs share the same JSON envelope. Node kinds:

- City: `building_params`, `building_mesh`, `place_along_path`, `fill_grid`, `merge_instances`, `city_root`
- Smoke: `smoke_domain`, `smoke_source`, `smoke_solver`, `smoke_root`

## Cook viewport (`cook`)

Returns city mesh buffers and/or smoke volume data. Smoke graphs also include a native **wgpu** raymarch preview image.

```json
{
  "stats": {
    "vertex_count": 0,
    "instance_count": 49152,
    "output_kind": "smoke",
    "graph_name": "Smoke Plume"
  },
  "viewport": {
    "output_kind": "smoke",
    "mesh": null,
    "smoke": {
      "nx": 32,
      "ny": 48,
      "nz": 32,
      "density": [0.0, 0.01, ...],
      "max_density": 2.5
    }
  },
  "smoke_preview": {
    "width": 640,
    "height": 480,
    "rgba": [10, 12, 18, 255, ...]
  }
}
```

City graphs return `output_kind: "city"` with instanced mesh buffers (unchanged from Alpha 2).

## Smoke volume export (`export_smoke_volume`)

Writes `elfentier_smoke_v1`: 16-byte header (`u32 nx, ny, nz, frame`) + little-endian `f32` density samples (x-fastest). Suitable for Unity `Texture3D` import or flipbook atlasing.

## Prompt interpreter (`apply_prompt_command`)

| Phrase | Effect |
|--------|--------|
| `商店街` / `shop street` | Load shop street preset |
| `煙` / `smoke` / `smoke plume` | Load smoke plume preset |
| `階数を5に` / `5 floors` | Set floor count |
| `seed変え` / `change seed` | Randomize seed |
| `もっと窓` / `more windows` | Increase window density |
| `グリッド配置` / `grid placement` | Switch to grid placement node |

## Workflow

1. **Graph is source of truth** — prompts only mutate the graph JSON.
2. **Cook is deterministic** — same graph always yields the same output.
3. Agents should call `apply_prompt_command` → `cook` → `export_gltf` or `export_smoke_volume`.
