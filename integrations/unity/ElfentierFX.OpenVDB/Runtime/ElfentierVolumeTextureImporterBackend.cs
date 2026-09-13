using System.IO;
using System.Threading.Tasks;
using UnityEngine;

namespace ElfentierFX.OpenVDB
{
    /// <summary>
    /// Phase 1 importer for <c>elfentier_volume_texture_v1</c> (<c>.evol</c>) files.
    /// </summary>
    public sealed class ElfentierVolumeTextureImporterBackend : IVdbImporter
    {
        public string[] SupportedExtensions => new[] { "evol" };

        public bool CanImport(string absolutePath)
        {
            var ext = Path.GetExtension(absolutePath)?.TrimStart('.').ToLowerInvariant();
            return ext == "evol" && File.Exists(absolutePath);
        }

        public Task<ElfentierVolumeImportResult> ImportAsync(string absolutePath, string assetFolder)
        {
            var header = ElfentierVolumeTextureReader.ReadHeader(absolutePath);
            var baseName = Path.GetFileNameWithoutExtension(absolutePath);
            var frames = ElfentierVolumeTextureReader.CreateFrameTextures(absolutePath, header, baseName);

            Directory.CreateDirectory(assetFolder);
            var savedFrames = new Texture3D[frames.Length];
            for (var i = 0; i < frames.Length; i++)
            {
                var assetPath = Path.Combine(assetFolder, $"{baseName}_frame{i:D3}.asset");
                savedFrames[i] = SaveTextureAsset(frames[i], assetPath);
            }

            var result = new ElfentierVolumeImportResult
            {
                VolumeTexture = savedFrames.Length > 0 ? savedFrames[0] : null,
                FrameTextures = savedFrames,
                BoundsMin = header.BoundsMin,
                BoundsMax = header.BoundsMax,
                PlaybackFps = 12f,
                SourceFormat = ElfentierVolumeHeader.Format,
                AssetPath = assetFolder,
            };

            return Task.FromResult(result);
        }

        static Texture3D SaveTextureAsset(Texture3D texture, string assetPath)
        {
#if UNITY_EDITOR
            var existing = UnityEditor.AssetDatabase.LoadAssetAtPath<Texture3D>(assetPath);
            if (existing != null)
            {
                UnityEditor.AssetDatabase.DeleteAsset(assetPath);
            }

            UnityEditor.AssetDatabase.CreateAsset(texture, assetPath);
            UnityEditor.AssetDatabase.SaveAssets();
            return UnityEditor.AssetDatabase.LoadAssetAtPath<Texture3D>(assetPath);
#else
            return texture;
#endif
        }
    }
}
