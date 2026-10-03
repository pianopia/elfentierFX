"""Render the previously validated ocean level through UE SceneCapture2D.
Run after ue_ocean.py, with ELFENTIER_RESULT and ELFENTIER_RENDER_DIR set.
The PNG is visual evidence, not a pixel-equivalence assertion.
"""
import json
import importlib.util
import os
from pathlib import Path
import unreal as ue

result=Path(os.environ["ELFENTIER_RESULT"])
data=json.loads(result.read_text(encoding="utf-8"))
assert data["ok"] and data["hlsl_compilation_verified"]
root=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location("three_import",root/"integrations/unreal/ElfentierFX/Content/Python/import_three_bundle.py")
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
module.refresh_native_materials(data)
ue.EditorLoadingAndSavingUtils.new_blank_map(False)
world=ue.get_editor_subsystem(ue.UnrealEditorSubsystem).get_editor_world()
subsystem=ue.get_editor_subsystem(ue.EditorActorSubsystem)
position,target_point=ue.Vector(2400,2800,750),ue.Vector(-600,-2100,20)
actors=[]
for path in data["assets"]:
    asset=ue.load_asset(path)
    if isinstance(asset,ue.StaticMesh):
        actor=subsystem.spawn_actor_from_class(ue.StaticMeshActor,ue.Vector())
        actor.static_mesh_component.set_static_mesh(asset)
        bound=actor.static_mesh_component.get_material(0)
        ue.log("OCEAN_BOUND_MATERIAL "+asset.get_name()+" "+str(bound.get_path_name() if bound else None))
        assert bound and bound.get_path_name() in data["materials"], "Native material assignment did not persist"
        actor.static_mesh_component.set_editor_property("bounds_scale",5.0)
        actors.append(actor)
capture=subsystem.spawn_actor_from_class(ue.SceneCapture2D,position)
capture.set_actor_rotation(ue.MathLibrary.find_look_at_rotation(position,target_point),False)
for actor in actors:
    if isinstance(actor,ue.StaticMeshActor):
        mesh=actor.static_mesh_component.static_mesh
        if mesh and mesh.get_name()=="sky":
            actor.set_actor_location(capture.get_actor_location(),False,False)
for path in data["materials"]:
    assert not ue.ElfentierStableFluidComponent.validate_material(ue.load_asset(path))
    directory=Path(os.environ["ELFENTIER_RENDER_DIR"])
    directory.mkdir(parents=True,exist_ok=True)
    (directory/(ue.load_asset(path).get_name()+".hlsl")).write_text(ue.ElfentierStableFluidComponent.get_material_source(ue.load_asset(path)),encoding="utf-8")
component=capture.get_component_by_class(ue.SceneCaptureComponent2D)
component.set_editor_property("fov_angle",49.0)
component.set_editor_property("capture_source",ue.SceneCaptureSource.SCS_FINAL_COLOR_LDR)
component.set_editor_property("capture_every_frame",False)
settings=ue.PostProcessSettings()
settings.set_editor_property("override_auto_exposure_method",True)
settings.set_editor_property("auto_exposure_method",ue.AutoExposureMethod.AEM_MANUAL)
settings.set_editor_property("override_auto_exposure_apply_physical_camera_exposure",True)
settings.set_editor_property("auto_exposure_apply_physical_camera_exposure",False)
settings.set_editor_property("override_auto_exposure_bias",True)
settings.set_editor_property("auto_exposure_bias",0.0)
component.set_editor_property("post_process_settings",settings)
target=ue.RenderingLibrary.create_render_target2d(world,1024,576,ue.TextureRenderTargetFormat.RTF_RGBA8)
component.set_editor_property("texture_target",target)
component.set_editor_property("always_persist_rendering_state",True)
assert ue.ElfentierStableFluidComponent.prepare_world_for_capture(world)
component.capture_scene()
# A readback flushes the render command before file export.
pixel=ue.RenderingLibrary.read_render_target_raw_pixel(world,target,512,288,False)
ue.log("OCEAN_CAPTURE_CENTER "+str(pixel))
sky_pixel=ue.RenderingLibrary.read_render_target_raw_pixel(world,target,512,64,False)
assert max(pixel.r+pixel.g+pixel.b,sky_pixel.r+sky_pixel.g+sky_pixel.b)>30, "Ocean capture is black"
directory=Path(os.environ["ELFENTIER_RENDER_DIR"])
directory.mkdir(parents=True,exist_ok=True)
ue.RenderingLibrary.export_render_target(world,target,str(directory),"ocean-native.png")
image=directory/"ocean-native.png"
assert image.is_file() and image.stat().st_size>1000
data["rendered_image"]=str(image)
assert ue.EditorLoadingAndSavingUtils.save_map(world,data["level"])
data["level_saved_verified"]=True
result.write_text(json.dumps(data,indent=2),encoding="utf-8")
ue.log("ELFENTIER_OCEAN_RENDER_PASS "+str(image))
