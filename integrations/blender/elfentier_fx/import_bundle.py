"""Import elfentierFX export bundle operator."""

import json
import os
import struct

import bpy
from bpy.props import StringProperty
from bpy.types import Operator
from mathutils import Vector

MANIFEST_FORMAT = "elfentier_export_manifest_v1"
LIQUID_FORMAT = "elfentier_liquid_cache_v1"


def load_manifest(bundle_dir: str) -> dict:
    path = os.path.join(bundle_dir, "manifest.json")
    with open(path, "r", encoding="utf-8") as handle:
        manifest = json.load(handle)
    if manifest.get("format") != MANIFEST_FORMAT:
        raise ValueError(f"Unsupported manifest format: {manifest.get('format')}")
    return manifest


def find_payload(manifest: dict, fmt: str) -> dict | None:
    for payload in manifest.get("payloads", []):
        if payload.get("format") == fmt:
            return payload
    return None


def import_glb(bundle_dir: str, relative_path: str, collection: bpy.types.Collection) -> None:
    glb_path = os.path.join(bundle_dir, relative_path)
    if not os.path.isfile(glb_path):
        raise FileNotFoundError(glb_path)

    if hasattr(bpy.ops.import_scene, "gltf"):
        bpy.ops.import_scene.gltf(filepath=glb_path)
    else:
        raise RuntimeError("glTF importer not available — enable the glTF 2.0 add-on")

    for obj in bpy.context.selected_objects:
        collection.objects.link(obj)
        for users_col in obj.users_collection:
            if users_col != collection:
                users_col.objects.unlink(obj)


def parse_liquid_cache(file_path: str) -> list[list[tuple[Vector, float]]]:
    frame_count = 0
    particles_per_frame = 0
    data_offset = 0

    with open(file_path, "rb") as handle:
        while True:
            line = handle.readline()
            if not line:
                break
            text = line.decode("utf-8", errors="replace").strip()
            if text.startswith("# frames="):
                parts = text[9:].split()
                if parts:
                    frame_count = int(parts[0])
                for token in parts:
                    if token.startswith("particles_per_frame="):
                        particles_per_frame = int(token.split("=", 1)[1])
            if not text.startswith("#"):
                data_offset = handle.tell() - len(line)
                break

    if frame_count <= 0 or particles_per_frame <= 0:
        return []

    frames: list[list[tuple[Vector, float]]] = []
    with open(file_path, "rb") as handle:
        handle.seek(data_offset)
        for _ in range(frame_count):
            frame: list[tuple[Vector, float]] = []
            for _ in range(particles_per_frame):
                x, y, z, radius = struct.unpack("<ffff", handle.read(16))
                frame.append((Vector((x, y, z)), radius))
            frames.append(frame)
    return frames


def import_liquid_frames(
    bundle_dir: str,
    relative_path: str,
    fps: float,
    collection: bpy.types.Collection,
) -> None:
    cache_path = os.path.join(bundle_dir, relative_path)
    frames = parse_liquid_cache(cache_path)
    if not frames:
        return

    mesh = bpy.data.meshes.new("ElfentierLiquidFrame0")
    first = frames[0]
    mesh.from_pydata([pos for pos, _ in first], [], [])
    for idx, (_, radius) in enumerate(first):
        mesh.vertices[idx].bevel_weight = radius

    obj = bpy.data.objects.new("ElfentierLiquid", mesh)
    collection.objects.link(obj)

    # Shape keys for remaining frames (point positions only).
    basis = obj.shape_key_add(name="Basis")
    basis.interpolation = "KEY_LINEAR"
    for frame_index, frame in enumerate(frames[1:], start=1):
        key = obj.shape_key_add(name=f"Frame{frame_index}")
        for vert, (pos, _) in zip(key.data, frame):
            vert.co = pos

    if obj.animation_data is None:
        obj.animation_data_create()
    action = bpy.data.actions.new(name="ElfentierLiquidPlayback")
    obj.animation_data.action = action

    for frame_index in range(len(frames)):
        key_name = "Basis" if frame_index == 0 else f"Frame{frame_index}"
        key_block = obj.data.shape_keys.key_blocks.get(key_name)
        if key_block is None:
            continue
        key_block.value = 0.0
        key_block.keyframe_insert(data_path="value", frame=1)
        key_block.value = 1.0
        key_block.keyframe_insert(data_path="value", frame=frame_index + 1)
        key_block.value = 0.0
        key_block.keyframe_insert(data_path="value", frame=frame_index + 2)

    scene = bpy.context.scene
    scene.render.fps = int(fps)
    scene.frame_end = max(scene.frame_end, len(frames))


class ELFENTIER_OT_import_bundle(Operator):
    bl_idname = "import_scene.elfentier_bundle"
    bl_label = "Import ElfentierFX Bundle"
    bl_options = {"REGISTER", "UNDO"}

    directory: StringProperty(subtype="DIR_PATH")

    def execute(self, context):
        bundle_dir = self.directory
        manifest = load_manifest(bundle_dir)
        collection = bpy.data.collections.new(f"ElfentierFX_{manifest.get('graph_name', 'Import')}")
        context.scene.collection.children.link(collection)

        mode = manifest.get("graph_mode")
        fps = float(manifest.get("frame_rate", 12.0))

        if mode == "city":
            payload = find_payload(manifest, "gltf_glb")
            rel = payload["path"] if payload else "city_mesh.glb"
            import_glb(bundle_dir, rel, collection)
        elif mode == "liquid":
            payload = find_payload(manifest, LIQUID_FORMAT)
            rel = payload["path"] if payload else "liquid_cache.raw"
            import_liquid_frames(bundle_dir, rel, fps, collection)
        elif mode == "smoke":
            payload = find_payload(manifest, "elfentier_smoke_atlas_v1")
            rel = payload["path"] if payload else "smoke_density.raw"
            text = bpy.data.curves.new("ElfentierSmokeStub", type="FONT")
            text.body = f"Smoke atlas: {rel}\nImport flipbook manually."
            obj = bpy.data.objects.new("ElfentierSmoke", text)
            collection.objects.link(obj)
        else:
            self.report({"WARNING"}, f"Unknown graph_mode: {mode}")

        self.report({"INFO"}, f"Imported ElfentierFX bundle ({mode})")
        return {"FINISHED"}

    def invoke(self, context, event):
        context.window_manager.fileselect_add(self)
        return {"RUNNING_MODAL"}


def menu_import(self, context):
    self.layout.operator(ELFENTIER_OT_import_bundle.bl_idname, text="ElfentierFX Bundle")


def register():
    bpy.utils.register_class(ELFENTIER_OT_import_bundle)
    bpy.types.TOPBAR_MT_file_import.append(menu_import)


def unregister():
    bpy.types.TOPBAR_MT_file_import.remove(menu_import)
    bpy.utils.unregister_class(ELFENTIER_OT_import_bundle)
