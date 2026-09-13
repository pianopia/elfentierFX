using System;
using System.Collections.Generic;
using System.IO;
using System.Text.RegularExpressions;
using UnityEngine;

namespace ElfentierFX
{
    /// <summary>
    /// Parsed elfentier_export_manifest_v1 document.
    /// </summary>
    public sealed class ExportManifest
    {
        public string Format;
        public string GraphName;
        public string GraphMode;
        public string Units;
        public string UpAxis;
        public float FrameRate;
        public List<ExportPayloadEntry> Payloads = new List<ExportPayloadEntry>();
    }

    public sealed class ExportPayloadEntry
    {
        public string Format;
        public string Path;
        public int FrameCount;
        public int ByteLen;
        public int TriangleCount;
    }

    public static class ExportManifestReader
    {
        public const string ManifestFormat = "elfentier_export_manifest_v1";

        public static ExportManifest Load(string bundleDirectory)
        {
            var manifestPath = Path.Combine(bundleDirectory, "manifest.json");
            if (!File.Exists(manifestPath))
            {
                throw new FileNotFoundException("manifest.json not found in bundle directory", manifestPath);
            }

            var json = File.ReadAllText(manifestPath);
            return Parse(json);
        }

        public static ExportManifest Parse(string json)
        {
            var manifest = new ExportManifest
            {
                Format = ReadString(json, "format"),
                GraphName = ReadString(json, "graph_name"),
                GraphMode = ReadString(json, "graph_mode"),
                Units = ReadString(json, "units"),
                UpAxis = ReadString(json, "up_axis"),
                FrameRate = ReadFloat(json, "frame_rate", 12f),
            };

            if (manifest.Format != ManifestFormat)
            {
                throw new InvalidDataException($"Unsupported manifest format: {manifest.Format}");
            }

            var payloadMatches = Regex.Matches(
                json,
                "\\{\\s*\"format\"\\s*:\\s*\"(?<format>[^\"]+)\"\\s*,\\s*\"path\"\\s*:\\s*\"(?<path>[^\"]+)\"[^}]*\\}",
                RegexOptions.Singleline);

            foreach (Match match in payloadMatches)
            {
                var block = match.Value;
                manifest.Payloads.Add(new ExportPayloadEntry
                {
                    Format = match.Groups["format"].Value,
                    Path = match.Groups["path"].Value,
                    FrameCount = ReadInt(block, "frame_count", 0),
                    ByteLen = ReadInt(block, "byte_len", 0),
                    TriangleCount = ReadInt(block, "triangle_count", 0),
                });
            }

            return manifest;
        }

        public static ExportPayloadEntry FindPayload(ExportManifest manifest, string format)
        {
            foreach (var payload in manifest.Payloads)
            {
                if (payload.Format == format)
                {
                    return payload;
                }
            }

            return null;
        }

        static string ReadString(string json, string key)
        {
            var match = Regex.Match(json, $"\"{key}\"\\s*:\\s*\"(?<value>[^\"]*)\"");
            return match.Success ? match.Groups["value"].Value : string.Empty;
        }

        static float ReadFloat(string json, string key, float fallback)
        {
            var match = Regex.Match(json, $"\"{key}\"\\s*:\\s*(?<value>-?[0-9.]+)");
            return match.Success && float.TryParse(match.Groups["value"].Value, out var value)
                ? value
                : fallback;
        }

        static int ReadInt(string json, string key, int fallback)
        {
            var match = Regex.Match(json, $"\"{key}\"\\s*:\\s*(?<value>-?[0-9]+)");
            return match.Success && int.TryParse(match.Groups["value"].Value, out var value)
                ? value
                : fallback;
        }
    }
}
