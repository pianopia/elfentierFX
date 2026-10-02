"""Import three-to-unreal bundles as UE native assets (Editor, UE 5.3+).

Call import_bundle('/absolute/bundle', '/Game/ThreeDemo'). No source code or HLSL
is executed by this Python module; HLSL is compiled by UE's material compiler.
Pure validation runs without unreal for CI and before any Editor mutations.
"""
import json
import math
from pathlib import Path
import re
import struct

SUPPORTED = {"gltf_glb", "alembic_geometry_cache", "openvdb_sequence",
             "elfentier_volume_texture_v1", "elfentier_surface_cache_v1"}


def payload_path(root, relative):
    if not isinstance(relative, str) or "\\" in relative or Path(relative).is_absolute():
        raise ValueError("Expected relative POSIX payload path")
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f"Payload escapes bundle: {relative}")
    if not path.is_file() or path.stat().st_size == 0:
        raise ValueError(f"Missing/empty payload: {relative}")
    return path


def validate_bundle(bundle_dir, allow_pending=False):
    root = Path(bundle_dir).resolve()
    with (root / "manifest.json").open(encoding="utf-8") as handle:
        manifest = json.load(handle)
    if manifest.get("format") != "elfentier_three_unreal_v1" or manifest.get("version") != 1:
        raise ValueError("Unsupported three-to-unreal bundle version")
    if manifest.get("units") != "meters" or manifest.get("up_axis") != "Y":
        raise ValueError("Expected meter/Y-up source data")
    fps = manifest.get("frame_rate")
    if not isinstance(fps, (int, float)) or not math.isfinite(fps) or not 0 < fps <= 240:
        raise ValueError("Invalid frame rate")
    if any(d.get("severity") == "error" for d in manifest.get("diagnostics", [])):
        raise ValueError("Bundle contains unsupported conversions; inspect conversion-report.json")
    payloads = manifest.get("payloads", [])
    if not any(p.get("format") == "gltf_glb" for p in payloads):
        raise ValueError("Bundle contains no GLB")
    for payload in payloads:
        fmt = payload.get("format")
        if fmt not in SUPPORTED:
            raise ValueError(f"Unsupported payload: {fmt}")
        full = payload_path(root, payload.get("path"))
        if payload.get("byte_len") is not None and full.stat().st_size != payload["byte_len"]:
            raise ValueError(f"Payload size mismatch: {payload['path']}")
        if fmt == "gltf_glb":
            with full.open("rb") as handle:
                header = handle.read(20)
            if len(header) != 20:
                raise ValueError("Truncated GLB header")
            magic, version, length, chunk_length, chunk_type = struct.unpack("<4sIIII", header)
            if magic != b"glTF" or version != 2 or length != full.stat().st_size or chunk_type != 0x4E4F534A or chunk_length > length - 20:
                raise ValueError("Invalid GLB header")
        if payload.get("status") and payload["status"] != "baked" and not allow_pending:
            raise ValueError(f"Native bake pending: {payload['path']}; pass --blender / --vdb-converter first")
        if fmt == "openvdb_sequence":
            paths = payload.get("paths", [])
            if len(paths) != payload.get("frame_count") or not paths or paths[0] != payload["path"]:
                raise ValueError("Invalid VDB sequence")
            for path in paths:
                full = payload_path(root, path)
                with full.open("rb") as handle:
                    magic = handle.read(8)
                if magic != bytes.fromhex("2042445600000000"):
                    raise ValueError("Invalid OpenVDB magic")
            if payload.get("units") != "centimeters" or payload.get("up_axis") != "Z":
                raise ValueError("VDB must be baked with --ue-space")
    names = set()
    for recipe in manifest.get("materials", []):
        name = recipe.get("name", "")
        if not re.fullmatch(r"[A-Za-z_]\w{0,127}", name) or name in names:
            raise ValueError("Invalid or duplicate material name")
        names.add(name)
        if recipe.get("kind") not in {"gltf_pbr", "custom_expression", "water_wpo"}:
            raise ValueError("Unsupported material recipe")
        if recipe["kind"] != "gltf_pbr":
            if recipe.get("output") != ("float4" if recipe["kind"] == "custom_expression" else "float3"):
                raise ValueError("Invalid material output dimensions")
            if not isinstance(recipe.get("hlsl"), str) or len(recipe["hlsl"]) > 65536:
                raise ValueError("Invalid HLSL recipe")
            inputs = recipe.get("inputs", [])
            input_names = set()
            for node in inputs:
                if not re.fullmatch(r"[A-Za-z_]\w*", node.get("name", "")) or node["name"] in input_names:
                    raise ValueError("Invalid custom input name")
                input_names.add(node["name"])
                if node.get("kind") not in {"uv", "time", "world_position", "scalar", "vector"}:
                    raise ValueError("Unsupported custom input")
                if node["kind"] in {"scalar", "vector"}:
                    values = [node.get("value")] if node["kind"] == "scalar" else node.get("value", [])
                    if not values or (node["kind"] == "vector" and len(values) != 4) or not all(isinstance(v, (int, float)) and math.isfinite(v) for v in values):
                        raise ValueError("Invalid parameter default")
    return manifest


def import_asset(filename, destination, options=None):
    import unreal
    task = unreal.AssetImportTask()
    task.set_editor_property("filename", str(filename))
    task.set_editor_property("destination_path", destination)
    task.set_editor_property("automated", True)
    task.set_editor_property("replace_existing", False)
    task.set_editor_property("save", True)
    if options:
        task.set_editor_property("options", options)
    unreal.AssetToolsHelpers.get_asset_tools().import_asset_tasks([task])
    paths = list(task.get_editor_property("imported_object_paths"))
    if not paths:
        raise RuntimeError(f"UE imported no assets from {filename}; check Interchange/Alembic/SVT plugins and the Output Log")
    return paths


def create_native_material(recipe, destination):
    import unreal as ue
    lib = ue.MaterialEditingLibrary
    asset = ue.AssetToolsHelpers.get_asset_tools().create_asset(
        "M_" + recipe["name"], destination, ue.Material, ue.MaterialFactoryNew())
    if not asset:
        raise RuntimeError(f"Cannot create material: {recipe['name']}")
    def node(cls):
        result = lib.create_material_expression(asset, cls)
        if not result:
            raise RuntimeError(f"Cannot create material node {cls}")
        return result
    custom = node(ue.MaterialExpressionCustom)
    custom.set_editor_property("code", recipe["hlsl"])
    custom.set_editor_property("output_type", ue.CustomMaterialOutputType.CMOT_FLOAT4
                               if recipe["output"] == "float4" else ue.CustomMaterialOutputType.CMOT_FLOAT3)
    pins = []
    for source in recipe["inputs"]:
        pin = ue.CustomInput()
        pin.set_editor_property("input_name", source["name"])
        pins.append(pin)
    custom.set_editor_property("inputs", pins)
    for source in recipe["inputs"]:
        kind = source["kind"]
        cls = {"uv": ue.MaterialExpressionTextureCoordinate, "time": ue.MaterialExpressionTime,
               "world_position": ue.MaterialExpressionWorldPosition,
               "scalar": ue.MaterialExpressionScalarParameter,
               "vector": ue.MaterialExpressionVectorParameter}[kind]
        expression = node(cls)
        output = ""
        if kind in {"scalar", "vector"}:
            expression.set_editor_property("parameter_name", source["name"])
            value = source["value"]
            expression.set_editor_property("default_value", value if kind == "scalar" else ue.LinearColor(*value))
            # Default vector output is RGB; explicitly include alpha for float4 uniforms.
            if kind == "vector":
                append = node(ue.MaterialExpressionAppendVector)
                if not lib.connect_material_expressions(expression, "RGB", append, "A") or not lib.connect_material_expressions(expression, "A", append, "B"):
                    raise RuntimeError("Cannot connect vector parameter components")
                expression = append
        if not lib.connect_material_expressions(expression, output, custom, source["name"]):
            raise RuntimeError(f"Cannot connect Custom input {source['name']}")
    def connect(expression, prop):
        if not lib.connect_material_property(expression, "", prop):
            raise RuntimeError(f"Cannot connect material property {prop}")
    if recipe["kind"] == "water_wpo":
        connect(custom, ue.MaterialProperty.MP_WORLD_POSITION_OFFSET)
        color = node(ue.MaterialExpressionConstant3Vector)
        color.set_editor_property("constant", ue.LinearColor(*recipe["color"], 1.0))
        connect(color, ue.MaterialProperty.MP_BASE_COLOR)
        roughness = node(ue.MaterialExpressionConstant)
        roughness.set_editor_property("r", recipe["roughness"])
        connect(roughness, ue.MaterialProperty.MP_ROUGHNESS)
    else:
        asset.set_editor_property("shading_model", ue.MaterialShadingModel.MSM_UNLIT)
        asset.set_editor_property("two_sided", recipe.get("twoSided", False))
        rgb = node(ue.MaterialExpressionComponentMask)
        for channel in ("r", "g", "b", "a"):
            rgb.set_editor_property(channel, channel != "a")
        lib.connect_material_expressions(custom, "", rgb, "Input")
        connect(rgb, ue.MaterialProperty.MP_EMISSIVE_COLOR)
        if recipe.get("transparent") or recipe.get("alphaTest", 0) > 0:
            alpha = node(ue.MaterialExpressionComponentMask)
            for channel in ("r", "g", "b", "a"):
                alpha.set_editor_property(channel, channel == "a")
            lib.connect_material_expressions(custom, "", alpha, "Input")
            masked = recipe.get("alphaTest", 0) > 0
            asset.set_editor_property("blend_mode", ue.BlendMode.BLEND_MASKED if masked else ue.BlendMode.BLEND_TRANSLUCENT)
            if masked:
                asset.set_editor_property("opacity_mask_clip_value", recipe["alphaTest"])
            connect(alpha, ue.MaterialProperty.MP_OPACITY_MASK if masked else ue.MaterialProperty.MP_OPACITY)
    lib.recompile_material(asset)
    ue.EditorAssetLibrary.save_loaded_asset(asset)
    return asset


def import_bundle(bundle_dir, destination="/Game/ElfentierThree", allow_pending=False):
    manifest = validate_bundle(bundle_dir, allow_pending)
    import unreal as ue
    if not re.fullmatch(r"/Game/[A-Za-z0-9_/]+", destination) or "//" in destination:
        raise ValueError("Destination must be a /Game content path")
    if ue.EditorAssetLibrary.does_directory_exist(destination) and ue.EditorAssetLibrary.list_assets(destination, recursive=True):
        raise ValueError("Destination must be empty; existing assets will not be overwritten")
    root = Path(bundle_dir).resolve()
    assets = []
    for payload in manifest["payloads"]:
        fmt = payload["format"]
        if fmt in {"elfentier_volume_texture_v1", "elfentier_surface_cache_v1"}:
            continue
        options = None
        if fmt == "alembic_geometry_cache":
            options = ue.AbcImportSettings()
            options.set_editor_property("import_type", ue.AlembicImportType.GEOMETRY_CACHE)
            conversion = ue.AbcConversionSettings()
            conversion.set_editor_property("preset", ue.AbcConversionPreset.CUSTOM)
            conversion.set_editor_property("scale", ue.Vector(100.0, -100.0, 100.0))
            conversion.set_editor_property("rotation", ue.Vector(90.0, 0.0, 0.0))
            options.set_editor_property("conversion_settings", conversion)
            sampling = ue.AbcSamplingSettings()
            sampling.set_editor_property("frame_start", 1)
            sampling.set_editor_property("frame_end", payload["frame_count"])
            options.set_editor_property("sampling_settings", sampling)
        if fmt == "openvdb_sequence":
            # SVT channel assignments/sequence selection are version-dependent UI settings.
            # Import every frame explicitly as static SVTs, preserving all frames.
            for index, path in enumerate(payload["paths"]):
                volume_id = re.sub(r"[^A-Za-z0-9_]", "_", Path(payload["path"]).parent.name)
                assets.extend(import_asset(payload_path(root, path), f"{destination}/Volumes/{volume_id}/Frame_{index:04}"))
            ue.log_warning("[ElfentierFX] VDB frames imported separately. Use Import Sequence for animated SVT, then assign a Volume-domain material to HeterogeneousVolume.")
        else:
            assets.extend(import_asset(payload_path(root, payload["path"]), destination + ("/Meshes" if fmt == "gltf_glb" else "/Surfaces"), options))
    materials = {}
    assets = list(dict.fromkeys(assets))
    for recipe in manifest["materials"]:
        if recipe["kind"] != "gltf_pbr":
            materials[recipe["name"]] = create_native_material(recipe, destination + "/Materials")
    assigned = {name: 0 for name in materials}
    for path in assets:
        mesh = ue.load_asset(path)
        if not isinstance(mesh, ue.StaticMesh):
            continue
        for index, slot in enumerate(mesh.get_editor_property("static_materials")):
            # imported_material_slot_name is editor-only C++ data and not exposed
            # by Python in all UE versions (including 5.8).
            names = {str(slot.material_slot_name)}
            if slot.material_interface:
                names.add(slot.material_interface.get_name())
            for name, material in materials.items():
                if name in names or "M_" + name in names:
                    mesh.set_material(index, material)
                    assigned[name] += 1
        ue.EditorAssetLibrary.save_loaded_asset(mesh)
    missing = [name for name, count in assigned.items() if not count]
    if missing:
        raise RuntimeError(f"Native materials were created but not assigned to imported mesh slots: {missing}")
    result = {"assets": assets, "materials": [m.get_path_name() for m in materials.values()], "assigned": assigned}
    ue.log("ELFENTIER_THREE_IMPORT_OK " + json.dumps(result))
    return result
