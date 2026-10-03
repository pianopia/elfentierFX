"""Native asset and material integration test; run with real RHI to compile HLSL.
Rendered equivalence is a separate check. The fixture covers sky/sand/water.
"""
import importlib.util
import json
import os
from pathlib import Path
import traceback
import uuid
import unreal as ue

root = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("three_import", root / "integrations/unreal/ElfentierFX/Content/Python/import_three_bundle.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
result = Path(os.environ["ELFENTIER_RESULT"])
try:
    ue.EditorLoadingAndSavingUtils.new_blank_map(False)
    destination = "/Game/OceanNative_" + uuid.uuid4().hex[:8]
    imported = module.import_bundle(os.environ["ELFENTIER_BUNDLE"], destination)
    assert len(imported["materials"]) == 3
    assert all(count > 0 for count in imported["assigned"].values())
    meshes = [ue.load_asset(path) for path in imported["assets"] if isinstance(ue.load_asset(path), ue.StaticMesh)]
    assert len(meshes) == 3
    assert sum(isinstance(ue.load_asset(path), ue.Texture2D) for path in imported["assets"]) == 2
    for path in imported["materials"]:
        errors=ue.ElfentierStableFluidComponent.validate_material(ue.load_asset(path))
        assert not errors,(path,errors)
    world = ue.get_editor_subsystem(ue.UnrealEditorSubsystem).get_editor_world()
    for mesh in meshes:
        actor = ue.get_editor_subsystem(ue.EditorActorSubsystem).spawn_actor_from_class(ue.StaticMeshActor, ue.Vector())
        actor.static_mesh_component.set_static_mesh(mesh)
        actor.static_mesh_component.set_editor_property("bounds_scale", 5.0)
        actor.set_actor_label(mesh.get_name())
    position, target = ue.Vector(2400,2800,750), ue.Vector(-600,-2100,20)
    capture = ue.get_editor_subsystem(ue.EditorActorSubsystem).spawn_actor_from_class(ue.SceneCapture2D, position)
    capture.set_actor_rotation(ue.MathLibrary.find_look_at_rotation(position,target), False)
    component = capture.get_component_by_class(ue.SceneCaptureComponent2D)
    component.set_editor_property("fov_angle",49.0)
    component.set_editor_property("capture_source",ue.SceneCaptureSource.SCS_FINAL_COLOR_LDR)
    component.set_editor_property("capture_every_frame",False)
    assert ue.EditorLoadingAndSavingUtils.save_map(world,destination+"/OceanTest")
    result.write_text(json.dumps({"ok":True,"rhi_required":True,"hlsl_compilation_verified":True,"visual_equivalence_verified":False,"level":destination+"/OceanTest",**imported},indent=2),encoding="utf-8")
    ue.log("ELFENTIER_OCEAN_NATIVE_IMPORT_PASS")
except Exception:
    result.write_text(json.dumps({"ok":False,"error":traceback.format_exc()},indent=2),encoding="utf-8")
    raise
