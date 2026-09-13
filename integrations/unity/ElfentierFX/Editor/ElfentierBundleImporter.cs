using System.IO;
using ElfentierFX;
using UnityEditor;
using UnityEngine;

namespace ElfentierFX.Editor
{
    public static class ElfentierBundleImporter
    {
        /// <summary>
        /// Optional hook registered by ElfentierFX.OpenVDB for <c>volume_texture.evol</c> playback.
        /// </summary>
        public static System.Action<string, ExportManifest, GameObject> VolumeImportHandler;

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
            var root = new GameObject($"ElfentierFX_{manifest.GraphName}");
            root.transform.position = Vector3.zero;

            Debug.Log(
                $"[ElfentierFX] Imported bundle '{manifest.GraphName}' mode={manifest.GraphMode} units={manifest.Units} up={manifest.UpAxis} fps={manifest.FrameRate.ToString("F1")}");

            switch (manifest.GraphMode)
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
                    Debug.LogWarning($"[ElfentierFX] Unknown graph_mode: {manifest.GraphMode}");
                    break;
            }

            Selection.activeGameObject = root;
        }

        static void ImportCityMesh(string bundleDirectory, ExportManifest manifest, Transform parent)
        {
            var payload = ExportManifestReader.FindPayload(manifest, "gltf_glb");
            var relative = payload?.Path ?? "city_mesh.glb";
            var glbPath = Path.Combine(bundleDirectory, relative);

            if (!File.Exists(glbPath))
            {
                Debug.LogError($"[ElfentierFX] Missing GLB at {glbPath}");
                return;
            }

            var destDir = Path.Combine("Assets", "ElfentierFX", Sanitize(manifest.GraphName));
            Directory.CreateDirectory(destDir);
            var destPath = Path.Combine(destDir, Path.GetFileName(glbPath));
            File.Copy(glbPath, destPath, true);
            AssetDatabase.Refresh();

            var placeholder = new GameObject("city_mesh");
            placeholder.transform.SetParent(parent, false);
            Debug.Log(
                $"[ElfentierFX] Copied GLB to {destPath}. Import with glTFast or drag into scene. tris={payload?.TriangleCount}");
        }

        static void ImportLiquidCache(string bundleDirectory, ExportManifest manifest, GameObject root)
        {
            var payload = ExportManifestReader.FindPayload(manifest, LiquidCacheReader.Format);
            var relative = payload?.Path ?? "liquid_cache.raw";
            var cachePath = Path.Combine(bundleDirectory, relative);

            if (!File.Exists(cachePath))
            {
                Debug.LogError($"[ElfentierFX] Missing liquid cache at {cachePath}");
                return;
            }

            var frames = LiquidCacheReader.Load(cachePath);
            var playback = root.AddComponent<ElfentierLiquidPlayback>();
            playback.SetFrames(frames, manifest.FrameRate);
            Debug.Log($"[ElfentierFX] Loaded {frames.Count} liquid frames from {cachePath}");
        }

        static void ImportSmokeAtlas(string bundleDirectory, ExportManifest manifest, Transform parent)
        {
            var payload = ExportManifestReader.FindPayload(manifest, "elfentier_smoke_atlas_v1");
            var relative = payload?.Path ?? "smoke_density.raw";
            var atlasPath = Path.Combine(bundleDirectory, relative);
            var marker = new GameObject("smoke_atlas");
            marker.transform.SetParent(parent, false);
            Debug.Log(
                $"[ElfentierFX] Smoke XY atlas at {atlasPath} frames={payload?.FrameCount}. Use OpenVDB package for 3D volume playback.");

            if (VolumeImportHandler != null)
            {
                VolumeImportHandler(bundleDirectory, manifest, parent.gameObject);
            }
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
