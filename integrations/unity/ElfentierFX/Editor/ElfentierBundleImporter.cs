using System.IO;
using ElfentierFX;
using UnityEditor;
using UnityEngine;

namespace ElfentierFX.Editor
{
    public static class ElfentierBundleImporter
    {
        const string MenuPath = "ElfentierFX/Import Export Bundle…";

        [MenuItem(MenuPath)]
        public static void ImportBundle()
        {
            var bundleDir = EditorUtility.OpenFolderPanel("Select elfentierFX export bundle", "", "");
            if (string.IsNullOrEmpty(bundleDir))
            {
                return;
            }

            ImportBundleAtPath(bundleDir);
        }

        public static void ImportBundleAtPath(string bundleDirectory)
        {
            var manifest = ExportManifestReader.Load(bundleDirectory);
            var root = new GameObject($"ElfentierFX_{manifest.graph_name}");
            root.transform.position = Vector3.zero;

            Debug.Log(
                $"[ElfentierFX] Imported bundle '{manifest.graph_name}' mode={manifest.graph_mode} units={manifest.units} up={manifest.up_axis} fps={manifest.frame_rate.ToString("F1")}");

            switch (manifest.graph_mode)
            {
                case "city":
                    ImportCityMesh(bundleDirectory, manifest, root.transform);
                    break;
                case "liquid":
                    ImportLiquidCache(bundleDirectory, manifest, root);
                    break;
                case "smoke":
                    ImportSmokeAtlas(bundleDirectory, manifest, root.transform);
                    break;
                default:
                    Debug.LogWarning($"[ElfentierFX] Unknown graph_mode: {manifest.graph_mode}");
                    break;
            }

            Selection.activeGameObject = root;
        }

        static void ImportCityMesh(string bundleDirectory, ExportManifest manifest, Transform parent)
        {
            var payload = ExportManifestReader.FindPayload(manifest, "gltf_glb");
            var relative = payload?.path ?? "city_mesh.glb";
            var glbPath = Path.Combine(bundleDirectory, relative);

            if (!File.Exists(glbPath))
            {
                Debug.LogError($"[ElfentierFX] Missing GLB at {glbPath}");
                return;
            }

            var destDir = Path.Combine("Assets", "ElfentierFX", Sanitize(manifest.graph_name));
            Directory.CreateDirectory(destDir);
            var destPath = Path.Combine(destDir, Path.GetFileName(glbPath));
            File.Copy(glbPath, destPath, true);
            AssetDatabase.Refresh();

            var placeholder = new GameObject("city_mesh");
            placeholder.transform.SetParent(parent, false);
            Debug.Log(
                $"[ElfentierFX] Copied GLB to {destPath}. Import with glTFast or drag into scene. tris={payload?.triangle_count}");
        }

        static void ImportLiquidCache(string bundleDirectory, ExportManifest manifest, GameObject root)
        {
            var payload = ExportManifestReader.FindPayload(manifest, LiquidCacheReader.Format);
            var relative = payload?.path ?? "liquid_cache.raw";
            var cachePath = Path.Combine(bundleDirectory, relative);

            if (!File.Exists(cachePath))
            {
                Debug.LogError($"[ElfentierFX] Missing liquid cache at {cachePath}");
                return;
            }

            var frames = LiquidCacheReader.Load(cachePath);
            var playback = root.AddComponent<ElfentierLiquidPlayback>();
            playback.SetFrames(frames, manifest.frame_rate);
            Debug.Log($"[ElfentierFX] Loaded {frames.Count} liquid frames from {cachePath}");
        }

        static void ImportSmokeAtlas(string bundleDirectory, ExportManifest manifest, Transform parent)
        {
            var payload = ExportManifestReader.FindPayload(manifest, "elfentier_smoke_atlas_v1");
            var relative = payload?.path ?? "smoke_density.raw";
            var atlasPath = Path.Combine(bundleDirectory, relative);
            var marker = new GameObject("smoke_atlas_stub");
            marker.transform.SetParent(parent, false);
            Debug.Log(
                $"[ElfentierFX] Smoke atlas stub at {atlasPath} frames={payload?.frame_count}. Wire to flipbook material manually.");
        }

        static string Sanitize(string name)
        {
            foreach (var c in Path.GetInvalidFileNameChars())
            {
                name = name.Replace(c, '_');
            }

            return name;
        }
    }
}
