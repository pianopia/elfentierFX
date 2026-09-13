using System.IO;
using ElfentierFX;
using UnityEditor;
using UnityEngine;

namespace ElfentierFX.OpenVDB.Editor
{
    /// <summary>
    /// Extends bundle import with volume texture playback when <c>volume_texture.evol</c> is present.
    /// </summary>
    [InitializeOnLoad]
    public static class ElfentierBundleVolumeBridge
    {
        public const string VolumeFormat = "elfentier_volume_texture_v1";

        static ElfentierBundleVolumeBridge()
        {
            ElfentierFX.Editor.ElfentierBundleImporter.VolumeImportHandler = ImportVolumeFromBundle;
        }

        static void ImportVolumeFromBundle(string bundleDirectory, ExportManifest manifest, GameObject root)
        {
            var payload = ExportManifestReader.FindPayload(manifest, VolumeFormat);
            var relative = payload?.Path ?? "volume_texture.evol";
            var volumePath = Path.Combine(bundleDirectory, relative);

            if (!File.Exists(volumePath))
            {
                Debug.LogWarning($"[ElfentierFX.OpenVDB] Bundle missing volume payload at {volumePath}");
                return;
            }

            try
            {
                var task = ElfentierVolumeImportService.ImportVolumeFileAsync(
                    volumePath,
                    manifest.GraphName,
                    root.transform);
                task.Wait();
            }
            catch (System.Exception ex)
            {
                Debug.LogError($"[ElfentierFX.OpenVDB] Bundle volume import failed: {ex.Message}");
            }
        }
    }
}
