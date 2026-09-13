using System.IO;
using System.Threading.Tasks;

namespace ElfentierFX.OpenVDB
{
    /// <summary>
    /// Placeholder for native OpenVDB <c>.vdb</c> import (Phase 2+).
    /// </summary>
    public sealed class NativeVdbImporterStub : IVdbImporter
    {
        public string[] SupportedExtensions => new[] { "vdb" };

        public bool CanImport(string absolutePath)
        {
            var ext = Path.GetExtension(absolutePath)?.TrimStart('.').ToLowerInvariant();
            return ext == "vdb";
        }

        public Task<ElfentierVolumeImportResult> ImportAsync(string absolutePath, string assetFolder)
        {
            throw new FileLoadException(
                "Native .vdb import is not available in Phase 1. " +
                "Export volume_texture.evol from elfentierFX (Export Bundle) or run " +
                "`cargo run -p vdb_convert -- from-smoke-preset <out.evol>`, then import the .evol file.");
        }
    }
}
