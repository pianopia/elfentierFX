"""Blender-side validation of the actual exported cache, including last frame."""
import json
import sys
from pathlib import Path
import bpy

source, abc = sys.argv[sys.argv.index("--") + 1:]
with open(source, encoding="utf-8") as handle:
    data = json.load(handle)
bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete(use_global=False)
bpy.ops.wm.alembic_import(filepath=str(Path(abc).resolve()), as_background_job=False)
mesh = next(o for o in bpy.context.scene.objects if o.type == "MESH")
bpy.context.scene.render.fps = round(data["fps"])
bpy.context.scene.render.fps_base = round(data["fps"]) / data["fps"]
for index in (0, len(data["frames"]) - 1):
    bpy.context.scene.frame_set(index + 1)
    evaluated = mesh.evaluated_get(bpy.context.evaluated_depsgraph_get())
    frame = data["frames"][index]
    assert len(evaluated.data.vertices) == len(frame) // 3
    for vertex, i in zip(evaluated.data.vertices, range(0, len(frame), 3)):
        expected = (frame[i], -frame[i + 2], frame[i + 1])
        assert all(abs(a - b) < 1e-5 for a, b in zip(vertex.co, expected)), (index, vertex.co[:], expected)
print("ELFENTIER_ALEMBIC_ROUNDTRIP_PASS")
