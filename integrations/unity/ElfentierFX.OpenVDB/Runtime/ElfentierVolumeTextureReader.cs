using System;
using System.IO;
using UnityEngine;

namespace ElfentierFX.OpenVDB
{
    /// <summary>
    /// Header for <c>elfentier_volume_texture_v1</c> (<c>.evol</c>) files.
    /// </summary>
    public sealed class ElfentierVolumeHeader
    {
        public const string Format = "elfentier_volume_texture_v1";
        public const int HeaderByteSize = 52;

        public int Nx;
        public int Ny;
        public int Nz;
        public int FrameCount;
        public int Channels;
        public Vector3 BoundsMin;
        public Vector3 BoundsMax;

        public int VoxelCount => Nx * Ny * Nz;
        public int FrameFloatCount => VoxelCount * Channels;
        public int DataByteLength => FrameFloatCount * FrameCount * 4;
    }

    /// <summary>
    /// Binary reader for elfentier volume texture payloads.
    /// </summary>
    public static class ElfentierVolumeTextureReader
    {
        public static ElfentierVolumeHeader ReadHeader(string filePath)
        {
            using var stream = File.OpenRead(filePath);
            using var reader = new BinaryReader(stream);
            var magic = reader.ReadBytes(4);
            if (magic.Length != 4 || magic[0] != (byte)'E' || magic[1] != (byte)'F' || magic[2] != (byte)'V' || magic[3] != (byte)'T')
            {
                throw new InvalidDataException("Invalid .evol magic — expected EFVT");
            }

            var version = reader.ReadUInt32();
            if (version != 1)
            {
                throw new InvalidDataException($"Unsupported .evol version {version}");
            }

            return new ElfentierVolumeHeader
            {
                Nx = reader.ReadInt32(),
                Ny = reader.ReadInt32(),
                Nz = reader.ReadInt32(),
                FrameCount = reader.ReadInt32(),
                Channels = reader.ReadInt32(),
                BoundsMin = new Vector3(reader.ReadSingle(), reader.ReadSingle(), reader.ReadSingle()),
                BoundsMax = new Vector3(reader.ReadSingle(), reader.ReadSingle(), reader.ReadSingle()),
            };
        }

        public static float[] ReadFrame(string filePath, int frameIndex)
        {
            var header = ReadHeader(filePath);
            if (frameIndex < 0 || frameIndex >= header.FrameCount)
            {
                throw new ArgumentOutOfRangeException(nameof(frameIndex));
            }

            var floatCount = header.FrameFloatCount;
            var data = new float[floatCount];
            using var stream = File.OpenRead(filePath);
            stream.Seek(ElfentierVolumeHeader.HeaderByteSize + frameIndex * floatCount * 4, SeekOrigin.Begin);
            using var reader = new BinaryReader(stream);
            for (var i = 0; i < floatCount; i++)
            {
                data[i] = reader.ReadSingle();
            }

            return data;
        }

        public static Texture3D CreateTexture3D(float[] density, ElfentierVolumeHeader header, string name)
        {
            var tex = new Texture3D(header.Nx, header.Ny, header.Nz, TextureFormat.RFloat, false)
            {
                name = name,
                wrapMode = TextureWrapMode.Clamp,
                filterMode = FilterMode.Bilinear,
            };

            var colors = new Color[density.Length];
            for (var i = 0; i < density.Length; i++)
            {
                colors[i] = new Color(density[i], 0f, 0f, 1f);
            }

            tex.SetPixels(colors);
            tex.Apply(updateMipmaps: false, makeNoLongerReadable: false);
            return tex;
        }

        public static Texture3D[] CreateFrameTextures(string filePath, ElfentierVolumeHeader header, string baseName)
        {
            var frames = new Texture3D[header.FrameCount];
            for (var f = 0; f < header.FrameCount; f++)
            {
                var density = ReadFrame(filePath, f);
                frames[f] = CreateTexture3D(density, header, $"{baseName}_frame{f:D3}");
            }

            return frames;
        }
    }
}
