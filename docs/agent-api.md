# elfentierFX Agent API

JSON-serializable Tauri commands for external agents and the desktop UI. All commands accept and return plain JSON — no API keys required for the local interpreter path.

## Commands

| Command | Input | Output | Description |
|---------|-------|--------|-------------|
| `get_core_version` | — | `string` | Core library semver |
| `get_graph` | `{ graph }` | `Graph` | Echo / validate a graph document |
| `list_presets_command` | — | `PresetInfo[]` | Built-in starter graphs |
| `load_preset` | `{ preset_id }` | `Graph` | Load preset by id (`shop_street`, `grid_block`) |
| `get_shop_street_preset` | — | `Graph` | Shop street starter graph |
| `set_params_command` | `{ request: { graph, node_id?, params } }` | `Graph` | Update `BuildingParams` on a node |
| `cook` | `{ graph }` | `CookWithMeshResult` | Cook graph + viewport mesh buffers |
| `cook_city_graph` | `{ graph }` | `CookResult` | Stats only (legacy) |
| `export_gltf` | `{ graph, path }` | `ExportResult` | Export merged `.glb` |
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

Node kinds: `building_params`, `building_mesh`, `place_along_path`, `fill_grid`, `merge_instances`, `city_root`.

## Cook viewport mesh (`cook`)

Returns instanced geometry for the 3D viewport:

```json
{
  "stats": {
    "vertex_count": 120,
    "index_count": 360,
    "triangle_count": 120,
    "instance_count": 6,
    "graph_name": "Shop Street"
  },
  "mesh": {
    "positions": [0.0, 0.0, 0.0, ...],
    "indices": [0, 1, 2, ...],
    "instance_matrices": [1,0,0,0, 0,1,0,0, ...],
    "vertex_count": 120,
    "triangle_count": 120,
    "instance_count": 6,
    "graph_name": "Shop Street"
  }
}
```

- `positions`: flat `f32` XYZ array (base building mesh, not merged).
- `indices`: `u32` triangle list.
- `instance_matrices`: column-major 4×4 per instance (16 floats each).

Export (`export_gltf`) still writes a **merged** mesh suitable for DCC/game engines.

## Prompt interpreter (`apply_prompt_command`)

Local rule-based mapping (JP + EN). Designed so a cloud LLM can later emit the same `GraphEdit` list.

Example phrases:

| Phrase | Effect |
|--------|--------|
| `商店街` / `shop street` | Load shop street preset |
| `階数を5に` / `5 floors` | Set floor count |
| `seed変え` / `change seed` | Randomize seed |
| `もっと窓` / `more windows` | Increase window density |
| `グリッド配置` / `grid placement` | Switch to grid placement node |

Response shape:

```json
{
  "graph": { "...": "updated Graph" },
  "intents": [{ "load_preset": { "preset_id": "shop_street" } }],
  "edits": [{ "load_preset": { "preset_id": "shop_street" } }],
  "summary": "Loaded preset shop_street"
}
```

## Workflow

1. **Graph is source of truth** — prompts only mutate the graph JSON.
2. **Cook is deterministic** — same graph always yields the same mesh.
3. Agents should call `apply_prompt_command` → `cook` → optionally `export_gltf`.

## Future

- MCP server wrapping these commands (not implemented in Alpha 2).
- Optional cloud LLM backend behind `apply_prompt` (stub only; no keys required today).
