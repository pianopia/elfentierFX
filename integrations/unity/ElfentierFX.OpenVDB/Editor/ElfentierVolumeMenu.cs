using System.IO;
using UnityEditor;
using UnityEngine;

namespace ElfentierFX.OpenVDB.Editor
{
    public static class ElfentierVolumeMenu
    {
        public const string ImportVolumeMenuPath = "ElfentierFX/Import Volume / OpenVDB…";

        [MenuItem(ImportVolumeMenuPath)]
        public static void ImportVolume()
        {
            var path = EditorUtility.OpenFilePanel(
                "Import elfentier volume or OpenVDB",
                "",
                "evol,vdb");

            if (string.IsNullOrEmpty(path))
            {
                return;
            }

            ImportVolumeAtPath(path);
        }

        public static void ImportVolumeAtPath(string absolutePath)
        {
            if (!File.Exists(absolutePath))
            {
                Debug.LogError($"[ElfentierFX.OpenVDB] File not found: {absolutePath}");
                return;
            }

            var ext = Path.GetExtension(absolutePath)?.TrimStart('.').ToLowerInvariant();
            if (ext == "vdb")
            {
                EditorUtility.DisplayDialog(
                    "OpenVDB import (Phase 1)",
                    "Native .vdb import is not available yet.\n\n" +
                    "Use elfentierFX Export Bundle (volume_texture.evol) or:\n" +
                    "cargo run -p vdb_convert -- from-smoke-preset <out.evol>\n\n" +
                    "Then import the .evol file with this menu.",
                    "OK");
                return;
            }

            try
            {
                var task = ElfentierVolumeImportService.ImportVolumeFileAsync(absolutePath);
                task.Wait();
                if (task.Result != null)
                {
                    Selection.activeGameObject = task.Result;
                }
            }
            catch (System.Exception ex)
            {
                Debug.LogError($"[ElfentierFX.OpenVDB] Import failed: {ex.Message}");
            }
        }
    }
}
