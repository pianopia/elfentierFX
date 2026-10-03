"""GPU execution proof, reading actual RGBA32f native render targets.
ELFENTIER_GPU_PLUGIN points to the generated stable-fluid plugin directory.
"""
import json
import os
from pathlib import Path
import traceback
import unreal as ue

result=Path(os.environ["ELFENTIER_RESULT"])
try:
    expected=json.loads((Path(os.environ["ELFENTIER_GPU_PLUGIN"])/"expected-reference.json").read_text(encoding="utf-8"))
    component=ue.ElfentierStableFluidComponent()
    assert component.reset()
    assert not component.step(-1,1)
    assert component.step(expected["delta"],expected["iterations"])
    world=ue.get_editor_subsystem(ue.UnrealEditorSubsystem).get_editor_world()
    maximum=0.0; samples=0
    for name,values in expected["state"].items():
        target=component.get_output(name)
        assert target is not None
        for y in range(expected["height"]):
            for x in range(expected["width"]):
                pixel=ue.RenderingLibrary.read_render_target_raw_pixel(world,target,x,y,False)
                rgba=[pixel.r,pixel.g,pixel.b,pixel.a]
                offset=4*(y*expected["width"]+x)
                for channel in range(4):
                    difference=abs(rgba[channel]-values[offset+channel])
                    maximum=max(maximum,difference)
                    assert difference<2e-5,(name,x,y,channel,rgba[channel],values[offset+channel])
                    samples+=1
    result.write_text(json.dumps({"ok":True,"variables":list(expected["state"]),"iterations":expected["iterations"],"samples":samples,"max_error":maximum},indent=2),encoding="utf-8")
    ue.log("ELFENTIER_NATIVE_GPU_EXECUTION_PASS")
except Exception:
    result.write_text(json.dumps({"ok":False,"error":traceback.format_exc()},indent=2),encoding="utf-8")
    raise
