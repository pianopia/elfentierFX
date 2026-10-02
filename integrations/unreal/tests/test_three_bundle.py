import importlib.util
import json
from pathlib import Path
import tempfile
import struct
import unittest

ENTRY = Path(__file__).parents[1] / "ElfentierFX/Content/Python/import_three_bundle.py"
spec = importlib.util.spec_from_file_location("three_import", ENTRY)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class BundleValidationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "scene.glb").write_bytes(struct.pack("<4sIIII", b"glTF", 2, 24, 4, 0x4E4F534A) + b"{}  ")
        self.manifest = {"format": "elfentier_three_unreal_v1", "version": 1,
            "units": "meters", "up_axis": "Y", "frame_rate": 24,
            "payloads": [{"format": "gltf_glb", "path": "scene.glb", "byte_len": 24}],
            "materials": [], "diagnostics": []}

    def tearDown(self):
        self.temp.cleanup()

    def validate(self, **kwargs):
        (self.root / "manifest.json").write_text(json.dumps(self.manifest), encoding="utf-8")
        return module.validate_bundle(self.root, **kwargs)

    def test_valid_without_unreal(self):
        self.assertEqual(self.validate()["version"], 1)

    def test_traversal(self):
        self.manifest["payloads"][0]["path"] = "../scene.glb"
        with self.assertRaisesRegex(ValueError, "escapes"):
            self.validate()

    def test_incorrect_bytes(self):
        self.manifest["payloads"][0]["byte_len"] = 100
        with self.assertRaisesRegex(ValueError, "size mismatch"):
            self.validate()

    def test_conversion_errors(self):
        self.manifest["diagnostics"] = [{"severity": "error"}]
        with self.assertRaisesRegex(ValueError, "unsupported conversions"):
            self.validate()

    def test_pending_requires_opt_in(self):
        self.manifest["payloads"][0]["status"] = "requires_bake"
        with self.assertRaisesRegex(ValueError, "pending"):
            self.validate()
        self.validate(allow_pending=True)

    def test_missing_sequence_frame(self):
        self.manifest["payloads"].append({"format": "openvdb_sequence", "path": "scene.glb", "paths": ["scene.glb"], "frame_count": 2})
        with self.assertRaisesRegex(ValueError, "sequence"):
            self.validate()

    def test_nonfinite_parameters(self):
        self.manifest["materials"] = [{"name": "Water", "kind": "custom_expression", "hlsl": "return float4(1,1,1,1);",
                                      "output": "float4",
                                      "inputs": [{"name": "Amount", "kind": "scalar", "value": float("nan")}]}]
        with self.assertRaisesRegex(ValueError, "default"):
            self.validate()


if __name__ == "__main__":
    unittest.main()
