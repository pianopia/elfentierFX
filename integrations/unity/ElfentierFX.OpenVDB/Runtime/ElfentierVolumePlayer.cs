using UnityEngine;

namespace ElfentierFX.OpenVDB
{
    /// <summary>
    /// Displays an imported 3D density texture in the scene via raymarch material.
    /// </summary>
    [ExecuteAlways]
    public sealed class ElfentierVolumePlayer : MonoBehaviour
    {
        [SerializeField] Texture3D volumeTexture;
        [SerializeField] Texture3D[] frameTextures;
        [SerializeField] Vector3 boundsMin = new Vector3(-4f, 0f, -4f);
        [SerializeField] Vector3 boundsMax = new Vector3(4f, 8f, 4f);
        [SerializeField] float playbackFps = 12f;
        [SerializeField] float densityScale = 1.5f;
        [SerializeField] float stepSize = 0.02f;
        [SerializeField] int maxSteps = 96;
        [SerializeField] Material volumeMaterial;

        MeshRenderer proxyRenderer;
        int currentFrame;
        float frameTimer;

        public Texture3D VolumeTexture => volumeTexture;

        public void Configure(
            Texture3D texture,
            Texture3D[] frames,
            Vector3 min,
            Vector3 max,
            float fps)
        {
            volumeTexture = texture;
            frameTextures = frames;
            boundsMin = min;
            boundsMax = max;
            playbackFps = fps;
            currentFrame = 0;
            frameTimer = 0f;
            ApplyMaterialProperties();
        }

        void OnEnable()
        {
            EnsureProxy();
            ApplyMaterialProperties();
        }

        void OnValidate()
        {
            ApplyMaterialProperties();
        }

        void Update()
        {
            if (frameTextures == null || frameTextures.Length == 0 || playbackFps <= 0f)
            {
                return;
            }

            frameTimer += Application.isPlaying ? Time.deltaTime : 0.016f;
            var interval = 1f / playbackFps;
            while (frameTimer >= interval)
            {
                frameTimer -= interval;
                currentFrame = (currentFrame + 1) % frameTextures.Length;
                volumeTexture = frameTextures[currentFrame];
                ApplyMaterialProperties();
            }
        }

        void EnsureProxy()
        {
            if (proxyRenderer != null)
            {
                return;
            }

            var proxy = GameObject.CreatePrimitive(PrimitiveType.Cube);
            proxy.name = "VolumeProxy";
            proxy.transform.SetParent(transform, false);
            var size = boundsMax - boundsMin;
            proxy.transform.localPosition = boundsMin + size * 0.5f;
            proxy.transform.localScale = size;
            var collider = proxy.GetComponent<Collider>();
            if (collider != null)
            {
                DestroyImmediate(collider);
            }

            proxyRenderer = proxy.GetComponent<MeshRenderer>();
            if (volumeMaterial == null)
            {
                var shader = Shader.Find("ElfentierFX/VolumeRaymarch");
                volumeMaterial = shader != null ? new Material(shader) : new Material(Shader.Find("Unlit/Color"));
            }

            proxyRenderer.sharedMaterial = volumeMaterial;
        }

        void ApplyMaterialProperties()
        {
            EnsureProxy();
            if (volumeMaterial == null || volumeTexture == null)
            {
                return;
            }

            volumeMaterial.SetTexture("_VolumeTex", volumeTexture);
            volumeMaterial.SetVector("_BoundsMin", boundsMin);
            volumeMaterial.SetVector("_BoundsMax", boundsMax);
            volumeMaterial.SetFloat("_DensityScale", densityScale);
            volumeMaterial.SetFloat("_StepSize", stepSize);
            volumeMaterial.SetFloat("_MaxSteps", maxSteps);
        }
    }
}
