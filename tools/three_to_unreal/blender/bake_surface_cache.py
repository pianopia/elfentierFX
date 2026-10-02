"""Blender background bridge: fixed-topology Y-up meter snapshots -> Alembic.

Run: blender -b --factory-startup --python bake_surface_cache.py -- input.json output.abc
No mesh cache is approximated with particles; shape-key frames retain all vertices.
"""
import json
import math
import os
import sys
import bpy


def bake(source, output):
    with open(source, encoding="utf-8") as handle:
        data = json.load(handle)
    if data.get("format") != "elfentier_surface_cache_v1":
        raise ValueError("Unsupported surface cache")
    fps = data["fps"]
    if not math.isfinite(fps) or fps <= 0:
        raise ValueError("Invalid frame rate")
    frames = data["frames"]
    indices = data["indices"]
    count = len(frames[0]) if frames else 0
    if not count or count % 3 or len(indices) % 3 or not indices:
        raise ValueError("Invalid mesh topology")
    if any(not isinstance(i, int) or i < 0 or i >= count // 3 for i in indices):
        raise ValueError("Invalid triangle index")
    if any(len(f) != count or not all(math.isfinite(v) for v in f) for f in frames):
        raise ValueError("Invalid vertex frames")
    if os.path.exists(output):
        raise FileExistsError(output)
    def vertices(frame):
        # Blender Z-up right-handed; Alembic exporter then emits Y-up right-handed.
        return [(frame[i], -frame[i + 2], frame[i + 1]) for i in range(0, count, 3)]
    mesh = bpy.data.meshes.new("FluidSurface")
    mesh.from_pydata(vertices(frames[0]), [], [indices[i:i + 3] for i in range(0, len(indices), 3)])
    mesh.update()
    obj = bpy.data.objects.new("FluidSurface", mesh)
    bpy.context.collection.objects.link(obj)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    obj.shape_key_add(name="Basis")
    for index, frame in enumerate(frames):
        key = obj.shape_key_add(name=f"Frame_{index:04}")
        for vertex, xyz in zip(key.data, vertices(frame)):
            vertex.co = xyz
        # Piecewise linear interpolation with one active key per sample.
        for sample, value in ((index, 0.0), (index + 1, 1.0), (index + 2, 0.0)):
            key.value = value
            key.keyframe_insert(data_path="value", frame=sample)
    # Ensure interpolation is linear (Blender 4.x and 5.x action API).
    action = mesh.shape_keys.animation_data.action
    if hasattr(action, "fcurves"):
        curves = action.fcurves
    else:
        slot = mesh.shape_keys.animation_data.action_slot
        curves = action.layers[0].strips[0].channelbag(slot).fcurves
    for curve in curves:
        for point in curve.keyframe_points:
            point.interpolation = "LINEAR"
    scene = bpy.context.scene
    scene.frame_start, scene.frame_end = 1, len(frames)
    scene.render.fps = round(fps)
    scene.render.fps_base = scene.render.fps / fps
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0
    os.makedirs(os.path.dirname(os.path.abspath(output)), exist_ok=True)
    bpy.ops.wm.alembic_export(filepath=os.path.abspath(output), selected=True,
        start=1, end=len(frames), xsamples=1, gsamples=1, global_scale=1.0,
        export_custom_properties=False, as_background_job=False)
    if not os.path.isfile(output) or os.path.getsize(output) == 0:
        raise RuntimeError("Alembic was not written")
    print(f"ELFENTIER_ALEMBIC_OK {output} frames={len(frames)} fps={fps}")


if __name__ == "__main__":
    try:
        args = sys.argv[sys.argv.index("--") + 1:]
        if len(args) != 2:
            raise ValueError("Expected input.json output.abc")
        bake(*args)
    except Exception:
        import traceback
        traceback.print_exc()
        # Blender otherwise exits successfully on many Python errors.
        sys.exit(1)
