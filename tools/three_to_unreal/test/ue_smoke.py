"""Run in an empty UE Editor project with -ExecutePythonScript=... .

ELFENTIER_BUNDLE = demo bundle; ELFENTIER_RESULT = output JSON path.
Verifies asset creation/assignment, not rendered visual equivalence.
"""
import importlib.util
import json
import os
from pathlib import Path
import traceback
import uuid
import unreal

root = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("three_import", root / "integrations/unreal/ElfentierFX/Content/Python/import_three_bundle.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
result = Path(os.environ["ELFENTIER_RESULT"])
try:
    destination = "/Game/ThreeSmoke_" + uuid.uuid4().hex[:8]
    imported = module.import_bundle(os.environ["ELFENTIER_BUNDLE"], destination,
                                   allow_pending=os.environ.get("ELFENTIER_ALLOW_PENDING") == "1")
    assert all(count > 0 for count in imported["assigned"].values())
    assert len(imported["materials"]) == 2
    assets = [unreal.load_asset(path) for path in imported["assets"]]
    assert any(isinstance(a, unreal.StaticMesh) for a in assets)
    assert any(isinstance(a, unreal.GeometryCache) for a in assets)
    assert sum(isinstance(a, unreal.SparseVolumeTexture) for a in assets) == 2
    for asset in assets:
        if isinstance(asset, unreal.GeometryCache):
            component = unreal.GeometryCacheComponent()
            component.set_geometry_cache(asset)
            assert component.get_duration() > 0
            assert component.get_number_of_frames() == 4
    vector = module.create_native_material({"name": "VectorProbe", "kind": "custom_expression",
        "hlsl": "return Tint;", "output": "float4",
        "inputs": [{"name": "Tint", "kind": "vector", "value": [0.1, 0.2, 0.3, 0.4]}]}, destination + "/Materials")
    value = unreal.MaterialEditingLibrary.get_material_default_vector_parameter_value(vector, "Tint")
    assert abs(value.a - 0.4) < 1e-5
    result.write_text(json.dumps({"ok": True, **imported}, indent=2), encoding="utf-8")
    unreal.log("ELFENTIER_UE_SMOKE_PASS")
except Exception:
    result.write_text(json.dumps({"ok": False, "error": traceback.format_exc()}, indent=2), encoding="utf-8")
    unreal.log_error(traceback.format_exc())
    raise
