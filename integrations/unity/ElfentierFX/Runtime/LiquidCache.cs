using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;

namespace ElfentierFX
{
    /// <summary>
    /// One frame of liquid particle positions and radii (meters, Y-up).
    /// </summary>
    public sealed class LiquidFrame
    {
        public Vector3[] Positions;
        public float[] Radii;
    }

    /// <summary>
    /// Parser for elfentier_liquid_cache_v1 raw files.
    /// </summary>
    public static class LiquidCacheReader
    {
        public const string Format = "elfentier_liquid_cache_v1";

        public static List<LiquidFrame> Load(string filePath)
        {
            var frames = new List<LiquidFrame>();
            using var stream = File.OpenRead(filePath);
            using var reader = new StreamReader(stream);

            var headerLines = 0;
            var frameCount = 0;
            var particlesPerFrame = 0;

            while (headerLines < 8)
            {
                var line = reader.ReadLine();
                if (line == null)
                {
                    break;
                }

                if (line.StartsWith("# frames=", StringComparison.Ordinal))
                {
                    var parts = line.Substring(9).Split(' ');
                    if (parts.Length >= 1 && int.TryParse(parts[0], out var fc))
                    {
                        frameCount = fc;
                    }

                    foreach (var token in parts)
                    {
                        if (token.StartsWith("particles_per_frame=", StringComparison.Ordinal))
                        {
                            int.TryParse(token.Substring("particles_per_frame=".Length), out particlesPerFrame);
                        }
                    }
                }

                if (!line.StartsWith("#", StringComparison.Ordinal))
                {
                    break;
                }

                headerLines++;
            }

            if (frameCount <= 0 || particlesPerFrame <= 0)
            {
                return frames;
            }

            using var binary = File.OpenRead(filePath);
            SkipHeader(binary);
            using var br = new BinaryReader(binary);

            for (var f = 0; f < frameCount; f++)
            {
                var positions = new Vector3[particlesPerFrame];
                var radii = new float[particlesPerFrame];

                for (var i = 0; i < particlesPerFrame; i++)
                {
                    var x = br.ReadSingle();
                    var y = br.ReadSingle();
                    var z = br.ReadSingle();
                    var radius = br.ReadSingle();
                    positions[i] = new Vector3(x, y, z);
                    radii[i] = radius;
                }

                frames.Add(new LiquidFrame { Positions = positions, Radii = radii });
            }

            return frames;
        }

        static void SkipHeader(FileStream stream)
        {
            stream.Seek(0, SeekOrigin.Begin);
            using var reader = new StreamReader(stream, leaveOpen: true);
            while (reader.Peek() >= 0)
            {
                var line = reader.ReadLine();
                if (line == null || !line.StartsWith("#", StringComparison.Ordinal))
                {
                    break;
                }
            }
        }
    }

    /// <summary>
    /// Minimal liquid cache playback using instanced sphere primitives.
    /// </summary>
    public sealed class ElfentierLiquidPlayback : MonoBehaviour
    {
        [SerializeField] float playbackFps = 12f;
        [SerializeField] Material particleMaterial;

        List<LiquidFrame> frames = new List<LiquidFrame>();
        int currentFrame;
        float frameTimer;
        readonly List<GameObject> instances = new List<GameObject>();

        public void SetFrames(IReadOnlyList<LiquidFrame> source, float fps)
        {
            frames = new List<LiquidFrame>(source);
            playbackFps = fps;
            currentFrame = 0;
            frameTimer = 0f;
            RebuildFrame();
        }

        void Update()
        {
            if (frames.Count == 0 || playbackFps <= 0f)
            {
                return;
            }

            frameTimer += Time.deltaTime;
            var interval = 1f / playbackFps;
            while (frameTimer >= interval)
            {
                frameTimer -= interval;
                currentFrame = (currentFrame + 1) % frames.Count;
                RebuildFrame();
            }
        }

        void RebuildFrame()
        {
            foreach (var go in instances)
            {
                if (go != null)
                {
                    Destroy(go);
                }
            }

            instances.Clear();

            if (frames.Count == 0)
            {
                return;
            }

            var frame = frames[currentFrame];
            var mat = particleMaterial != null ? particleMaterial : new Material(Shader.Find("Standard"));

            for (var i = 0; i < frame.Positions.Length; i++)
            {
                var radius = frame.Radii != null && i < frame.Radii.Length ? frame.Radii[i] : 0.1f;
                var sphere = GameObject.CreatePrimitive(PrimitiveType.Sphere);
                sphere.transform.SetParent(transform, false);
                sphere.transform.localPosition = frame.Positions[i];
                sphere.transform.localScale = Vector3.one * (radius * 2f);
                sphere.GetComponent<Renderer>().sharedMaterial = mat;
                instances.Add(sphere);
            }
        }
    }
}
