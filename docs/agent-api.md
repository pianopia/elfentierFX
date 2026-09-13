# elfentierFX Agent API

JSON-serializable Tauri commands for external agents and the desktop UI. All commands accept and return plain JSON — no API keys required for the local interpreter path.

## Commands

| Command | Input | Output | Description |
|---------|-------|--------|-------------|
| `get_core_version` | — | `string` | Core library semver |
| `get_graph` | `{ graph }` | `Graph` | Echo / validate a graph document |
| `list_presets_command` | — | `PresetInfo[]` | Built-in starter graphs |
| `load_preset` | `{ preset_id }` | `Graph` | Load preset by id |
| `get_shop_street_preset` | — | `Graph` | Shop street starter graph |
| `get_smoke_puff_preset` | — | `Graph` | Smoke/gas puff starter graph |
| `get_ocean_patch_preset` | — | `Graph` | Ocean patch FLIP liquid starter |
| `get_waterfall_preset` | — | `Graph` | Waterfall FLIP liquid starter |
| `get_flood_basin_preset` | — | `Graph` | Flood basin FLIP liquid starter |
| `set_params_command` | `{ request: { graph, node_id?, params } }` | `Graph` | Update `BuildingParams` on a node |
| `set_smoke_params_command` | `{ request: SetSmokeParamsRequest }` | `Graph` | Update smoke domain/source/solver/**collider** nodes |
| `set_liquid_params_command` | `{ request: SetLiquidParamsRequest }` | `Graph` | Update liquid domain/source/solver/**collider** nodes |
| `cook` | `{ graph }` | `CookWithMeshResult` | Cook graph + viewport buffers + native wgpu preview (wgpu-only; failures surface as `native_preview_error`) |
| `render_native_viewport_command` | `{ request: RenderNativeRequest }` | `NativePreviewImage` | Re-render mesh/smoke/liquid frame with camera (orbit/animation) |
| `cook_city_graph` | `{ graph }` | `CookResult` | Stats only (legacy) |
| `export_gltf` | `{ graph, path }` | `ExportResult` | Export merged city `.glb` |
| `export_smoke_density_command` | `{ request: { graph, path } }` | `SmokeExportResult` | Export smoke density XY atlas |
| `export_smoke_vdb_command` | `{ request: { graph, path } }` | `OpenVdbExportResult` | Export smoke fog `.vdb` (last frame) |
| `export_liquid_cache_command` | `{ request: { graph, path } }` | `LiquidExportResult` | Export liquid particle cache stub |
| `export_cook_bundle_command_handler` | `{ request: { graph, path? } }` | `ExportBundleResult` | Write manifest + payloads directory (temp path if `path` omitted) |
| `apply_prompt_command` | `{ request: { graph, prompt } }` | `ApplyPromptResult` | Local NL → graph edits |
| `explain_graph_command` | `{ graph }` | `string` | Template graph summary |

### Preset ids

`shop_street`, `grid_block`, `smoke_puff`, `smoke_viscous`, `smoke_sphere`, `ocean_patch`, `waterfall`, `flood_basin`, `liquid_ramp`

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

**Smoke:** `smoke_domain`, `smoke_source`, `smoke_collider`, `smoke_solver`, `smoke_root`

**Liquid (FLIP Phase 1+2):** `liquid_domain`, `liquid_source`, `liquid_collider`, `liquid_solver`, `liquid_root`

Smoke preset chain:

```
smoke_domain → smoke_source → smoke_collider → smoke_solver → smoke_root
```

Liquid preset chain:

```
liquid_domain → liquid_source → liquid_collider → liquid_solver → liquid_root
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
  "viscosity": 0.06,
  "pressure_iterations": 18,
  "ground_collision": true,
  "max_particles_per_frame": 1800
}
```

`smoke_collider` / `liquid_collider` (AABB or mesh SDF):

```json
{
  "enabled": true,
  "mode": "aabb",
  "bounds_min": { "x": -4, "y": 0, "z": -4 },
  "bounds_max": { "x": 4, "y": 0.35, "z": 4 },
  "bounce": 0.12,
  "kill_inside": true,
  "mesh_kind": "sphere",
  "mesh_resolution": 32,
  "position": { "x": 0, "y": 2.2, "z": 0 },
  "rotation_y": 0,
  "scale": { "x": 1.5, "y": 1.5, "z": 1.5 }
}
```

Mesh SDF example (`mode: "mesh_sdf"`):

```json
{
  "enabled": true,
  "mode": "mesh_sdf",
  "mesh_kind": "sphere",
  "mesh_resolution": 28,
  "position": { "x": 0, "y": 2.2, "z": 0 },
  "scale": { "x": 1.5, "y": 1.5, "z": 1.5 },
  "bounce": 0.12,
  "kill_inside": true,
  "bounds_min": { "x": -0.75, "y": 1.45, "z": -0.75 },
  "bounds_max": { "x": 0.75, "y": 2.95, "z": 0.75 }
}
```

Phase 2.5 notes: `mode` defaults to `aabb` when omitted. Mesh SDF uses a dense voxel grid built once per cook (not deforming/animated). Smoke zeros inward velocity along SDF gradient near the surface; liquid pushes particles out along the gradient (or ejects when `kill_inside`). Viewport cook returns mesh triangle wireframes for mesh SDF colliders. Accuracy is resolution-limited — see README Phase 2.5.

### Liquid params (on nodes)

`liquid_domain`:

```json
{
  "resolution": 22,
  "bounds_min": { "x": -8, "y": 0, "z": -8 },
  "bounds_max": { "x": 8, "y": 3.5, "z": 8 },
  "seed": 21,
  "initial_particles": 2400,
  "particle_radius": 0.14
}
```

`liquid_source`:

```json
{
  "position": { "x": 0, "y": 9, "z": 0 },
  "radius": 0.7,
  "emission_rate": 12,
  "velocity": { "x": 0, "y": -3.5, "z": 0 },
  "active_until_step": 0
}
```

`liquid_solver`:

```json
{
  "steps": 72,
  "frame_stride": 3,
  "gravity": 12,
  "flip_ratio": 0.97,
  "viscosity": 0.01,
  "pressure_iterations": 22,
  "wave_amplitude": 0,
  "wave_frequency": 1.2,
  "terrain_height": 0,
  "max_particles": 14000
}
```

## Cook viewport mesh (`cook`)

Returns instanced geometry for city graphs, animated smoke impostors, or animated liquid particles:

```json
{
  "stats": {
    "vertex_count": 0,
    "instance_count": 0,
    "graph_name": "Ocean Patch",
    "liquid_resolution": [22, 22, 22],
    "liquid_steps": 48,
    "liquid_frame_count": 13,
    "liquid_particle_count": 2400,
    "liquid_max_speed": 2.4
  },
  "mesh": {
    "positions": [],
    "indices": [],
    "instance_matrices": [1, 0, 0, 0, "..."],
    "liquid": {
      "frame_count": 13,
      "fps": 12,
      "bounds_min": [-8, 0, -8],
      "bounds_max": [8, 3.5, 8],
      "frames": [
        {
          "particle_count": 2400,
          "positions": [0.1, 1.2, 0.0, "..."],
          "radii": [0.14, "..."],
          "opacities": [0.7, "..."]
        }
      ],
      "stats": { "resolution": [22, 22, 22], "step_count": 48, "max_speed": 2.4 }
    }
  }
}
```

- City graphs: `positions` / `indices` / `instance_matrices` as before.
- Smoke graphs: `smoke.frames[]` holds soft particle impostors; viewport autoplays at `smoke.fps`.
- Liquid graphs: `liquid.frames[]` holds FLIP particles with radii; viewport autoplays at `liquid.fps`.
- Realtime preview is **wgpu-only**. On init/render failure, `native_preview` is null and `native_preview_error` carries the error string for the UI (no WebGL fallback).
- `native_preview` / `render_native_viewport_command` return `NativePreviewImage` with `rgba_base64` (standard base64 of `width * height * 4` RGBA8 bytes). The desktop UI decodes this payload and rejects truncated or missing buffers instead of drawing a blank canvas.

```json
{
  "width": 960,
  "height": 720,
  "rgba_base64": "…",
  "backend": "wgpu"
}
```

**Linux note:** headless CI/VMs may use llvmpipe for wgpu; preview pixels are still validated in Rust tests. Desktop users need a working Vulkan/Metal/DX12 driver for realtime orbit/animation.

Export (`export_gltf`) writes a **merged** mesh for city graphs only.

## Smoke density export (`export_smoke_density_command`)

Writes a small header plus raw `f32` density atlas bytes.

```json
{
  "path": "/tmp/elfentier_smoke_density.raw",
  "frame_count": 13,
  "byte_len": 4096,
  "format": "elfentier_smoke_atlas_v1"
}
```

Format: text header (`# elfentier smoke density atlas v1`) followed by little-endian `f32` data (XY slices, frame-major). Suitable for flipbook or 3D texture import in Unity.

## OpenVDB fog export (`export_smoke_vdb_command`)

Writes a standard OpenVDB `.vdb` archive with a `density` FloatGrid (fog volume class) from the latest smoke density frame.

```json
{
  "path": "/tmp/smoke_density.vdb",
  "grid_name": "density",
  "frame_index": 12,
  "byte_len": 68658,
  "format": "openvdb_fog_floatgrid_v1",
  "active_voxels": 5439,
  "resolution": [24, 24, 24]
}
```

Smoke export bundles also include `smoke_density.vdb` automatically. OpenVDB is a trademark of LF Projects, LLC.

## Liquid particle cache export (`export_liquid_cache_command`)

Unity-oriented stub: per-frame particle positions and radii.

```json
{
  "path": "/tmp/elfentier_liquid_cache.raw",
  "frame_count": 25,
  "byte_len": 384000,
  "format": "elfentier_liquid_cache_v1"
}
```

Format: text header (`# elfentier liquid particle cache v1`) followed by little-endian `f32` tuples `[x, y, z, radius]` per particle, frame-major. Import via Unity/Unreal/Blender integrations under `integrations/`.

## Cook export bundle (`export_cook_bundle_command_handler`)

Writes a bundle directory with unified manifest for engine/DCC import. See [`export-formats.md`](export-formats.md).

```json
{
  "directory": "/tmp/elfentier_export_ocean_patch_1726217280",
  "manifest_path": "/tmp/elfentier_export_ocean_patch_1726217280/manifest.json",
  "payload_count": 2,
  "payloads": [
    { "format": "elfentier_graph_v1", "path": "graph.json", "byte_len": 2048 },
    {
      "format": "elfentier_liquid_cache_v1",
      "path": "liquid_cache.raw",
      "frame_count": 20,
      "byte_len": 384000,
      "bounds_min": [-8.0, 0.0, -8.0],
      "bounds_max": [8.0, 3.5, 8.0],
      "resolution": [22, 22, 22]
    }
  ]
}
```

Manifest (`elfentier_export_manifest_v1`) fields:

| Field | Value |
|-------|-------|
| `units` | `meters` |
| `up_axis` | `Y` |
| `frame_rate` | `12.0` (fluid playback) |
| `graph_mode` | `city`, `smoke`, or `liquid` |

Payload files by mode:

| Mode | Primary payload |
|------|-----------------|
| city | `city_mesh.glb` (`gltf_glb`) |
| smoke | `smoke_density.raw` (`elfentier_smoke_atlas_v1`) |
| liquid | `liquid_cache.raw` (`elfentier_liquid_cache_v1`) |

Integrations: `integrations/unity/ElfentierFX/`, `integrations/unreal/ElfentierFX/`, `integrations/blender/elfentier_fx/`.

## Prompt interpreter (`apply_prompt_command`)

Local rule-based mapping (JP + EN). Designed so a cloud LLM can later emit the same `GraphEdit` list.

Example phrases:

| Phrase | Effect |
|--------|--------|
| `商店街` / `shop street` | Load shop street preset |
| `煙` / `smoke` / `smoke puff` | Load smoke puff preset |
| `海` / `ocean` / `ocean patch` | Load ocean patch preset |
| `滝` / `waterfall` | Load waterfall preset |
| `洪水` / `flood` / `flood basin` | Load flood basin preset |
| `水位上げ` / `more water` / `もっと水` | Increase liquid emission + steps |
| `もっと激しく` / `more intense` / `splashier` | Increase liquid intensity + waves |
| `もっと煙` / `more smoke` | Increase emission + simulation steps |
| `階数を5に` / `5 floors` | Set floor count |
| `seed変え` / `change seed` | Randomize seed |
| `もっと窓` / `more windows` | Increase window density |
| `グリッド配置` / `grid placement` | Switch to grid placement node |

Response shape:

```json
{
  "graph": { "...": "updated Graph" },
  "intents": [{ "load_preset": { "preset_id": "ocean_patch" } }],
  "edits": [{ "load_preset": { "preset_id": "ocean_patch" } }],
  "summary": "Loaded preset ocean_patch"
}
```

## Workflow

1. **Graph is source of truth** — prompts only mutate the graph JSON.
2. **Cook is deterministic** — same graph + seed always yields the same simulation.
3. Agents should call `apply_prompt_command` → `cook` → `export_cook_bundle_command_handler` (preferred) or individual export commands.

## Integrations (Phase 1)

| Target | Path |
|--------|------|
| Unity 6 | `integrations/unity/ElfentierFX/` |
| Unreal Engine | `integrations/unreal/ElfentierFX/` |
| Blender 4.x | `integrations/blender/elfentier_fx/` |

## Future

- MCP server wrapping these commands (not implemented in Alpha 2).
- Multi-frame OpenVDB sequences and liquid → VDB.
- Optional cloud LLM backend behind `apply_prompt` (stub only; no keys required today).
