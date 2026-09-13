using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using UnityEngine;

namespace ElfentierFX.OpenVDB.Editor
{
    /// <summary>
    /// Resolves the correct <see cref="IVdbImporter"/> and spawns scene objects.
    /// </summary>
    public static class ElfentierVolumeImportService
    {
        static readonly List<IVdbImporter> Importers = new List<IVdbImporter>
        {
            new ElfentierVolumeTextureImporterBackend(),
            new NativeVdbImporterStub(),
        };

        public static IVdbImporter ResolveImporter(string absolutePath)
        {
            return Importers.FirstOrDefault(importer => importer.CanImport(absolutePath));
        }

        public static async Task<GameObject> ImportVolumeFileAsync(
            string absolutePath,
            string graphName = null,
            Transform parent = null)
        {
            var importer = ResolveImporter(absolutePath);
            if (importer == null)
            {
                throw new FileLoadException($"No importer registered for {absolutePath}");
            }

            var safeName = string.IsNullOrEmpty(graphName)
                ? Path.GetFileNameWithoutExtension(absolutePath)
                : graphName;
            var assetFolder = Path.Combine("Assets", "ElfentierFX", "Volumes", Sanitize(safeName));
            var result = await importer.ImportAsync(absolutePath, assetFolder);

#if UNITY_EDITOR
            UnityEditor.AssetDatabase.Refresh();
#endif

            var root = new GameObject(parent == null ? $"ElfentierVolume_{Sanitize(safeName)}" : "volume_player");
            if (parent != null)
            {
                root.transform.SetParent(parent, false);
            }

            var player = root.AddComponent<ElfentierVolumePlayer>();
            player.Configure(
                result.VolumeTexture,
                result.FrameTextures,
                result.BoundsMin,
                result.BoundsMax,
                result.PlaybackFps);

            Debug.Log(
                $"[ElfentierFX.OpenVDB] Imported {result.SourceFormat} from {absolutePath} " +
                $"frames={result.FrameTextures?.Length ?? 0} assetFolder={assetFolder}");

            return root;
        }

        public static string Sanitize(string name)
        {
            foreach (var c in Path.GetInvalidFileNameChars())
            {
                name = name.Replace(c, '_');
            }

            return name;
        }
    }
}
