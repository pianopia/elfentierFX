using System.Threading.Tasks;
using UnityEngine;

namespace ElfentierFX.OpenVDB
{
    /// <summary>
    /// Pluggable volume import back end. Phase 1 implements elfentier <c>.evol</c> textures;
    /// native <c>.vdb</c> import will implement this interface in a later phase.
    /// </summary>
    public interface IVdbImporter
    {
        /// <summary>File extensions this importer handles (without dot), e.g. "evol", "vdb".</summary>
        string[] SupportedExtensions { get; }

        /// <summary>Whether this importer can handle the given absolute file path.</summary>
        bool CanImport(string absolutePath);

        /// <summary>
        /// Imports volume data and returns a scene-ready descriptor.
        /// Implementations may create assets under <paramref name="assetFolder"/>.
        /// </summary>
        Task<ElfentierVolumeImportResult> ImportAsync(string absolutePath, string assetFolder);
    }

    /// <summary>Result of a successful volume import.</summary>
    public sealed class ElfentierVolumeImportResult
    {
        public Texture3D VolumeTexture;
        public Texture3D[] FrameTextures;
        public Vector3 BoundsMin;
        public Vector3 BoundsMax;
        public float PlaybackFps = 12f;
        public string SourceFormat;
        public string AssetPath;
    }
}
