"""Editor utility: import elfentierFX export bundle metadata into Unreal.

Adjust BUNDLE_DIR before running from the Unreal Python console.
Live TCP bridge to elfentierFX: future work (see README).
"""

BUNDLE_DIR = "/tmp/elfentier_export_ocean_patch_0"


def load_manifest(bundle_dir: str) -> dict:
    import json
    import os

    manifest_path = os.path.join(bundle_dir, "manifest.json")
    with open(manifest_path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def import_bundle(bundle_dir: str) -> None:
    manifest = load_manifest(bundle_dir)
    print(
        f"[ElfentierFX] graph={manifest.get('graph_name')} mode={manifest.get('graph_mode')} "
        f"units={manifest.get('units')} up={manifest.get('up_axis')} fps={manifest.get('frame_rate')}"
    )

    for payload in manifest.get("payloads", []):
        fmt = payload.get("format")
        rel = payload.get("path")
        full = f"{bundle_dir}/{rel}"
        print(f"  payload {fmt} -> {full}")

        if fmt == "gltf_glb":
            print("    Import GLB via GLTFImporter or Interchange pipeline.")
        elif fmt == "elfentier_liquid_cache_v1":
            print("    Use FElfentierLiquidCacheLoader or Niagara CPU spawn from cache.")
        elif fmt == "elfentier_smoke_atlas_v1":
            print("    Wire density atlas to flipbook/subUV material.")


if __name__ == "__main__":
    import_bundle(BUNDLE_DIR)
