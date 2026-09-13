Shader "ElfentierFX/VolumeRaymarch"
{
    Properties
    {
        _VolumeTex ("Volume Density", 3D) = "" {}
        _BoundsMin ("Bounds Min", Vector) = (-4, 0, -4, 0)
        _BoundsMax ("Bounds Max", Vector) = (4, 8, 4, 0)
        _DensityScale ("Density Scale", Float) = 1.5
        _StepSize ("Step Size", Float) = 0.02
        _MaxSteps ("Max Steps", Float) = 96
        _Tint ("Tint", Color) = (0.85, 0.9, 1, 1)
    }
    SubShader
    {
        Tags { "Queue"="Transparent" "RenderType"="Transparent" }
        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        Cull Back

        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "UnityCG.cginc"

            sampler3D _VolumeTex;
            float3 _BoundsMin;
            float3 _BoundsMax;
            float _DensityScale;
            float _StepSize;
            float _MaxSteps;
            fixed4 _Tint;

            struct appdata
            {
                float4 vertex : POSITION;
            };

            struct v2f
            {
                float4 pos : SV_POSITION;
                float3 worldPos : TEXCOORD0;
            };

            v2f vert(appdata v)
            {
                v2f o;
                o.pos = UnityObjectToClipPos(v.vertex);
                o.worldPos = mul(unity_ObjectToWorld, v.vertex).xyz;
                return o;
            }

            float2 intersectAabb(float3 ro, float3 rd, float3 bmin, float3 bmax)
            {
                float3 invRd = 1.0 / (rd + 1e-6);
                float3 t0 = (bmin - ro) * invRd;
                float3 t1 = (bmax - ro) * invRd;
                float3 tmin = min(t0, t1);
                float3 tmax = max(t0, t1);
                float tn = max(max(tmin.x, tmin.y), tmin.z);
                float tf = min(min(tmax.x, tmax.y), tmax.z);
                return float2(tn, tf);
            }

            float sampleDensity(float3 worldPos)
            {
                float3 size = _BoundsMax - _BoundsMin;
                float3 uvw = (worldPos - _BoundsMin) / max(size, 1e-4);
                if (any(uvw < 0.0) || any(uvw > 1.0))
                {
                    return 0.0;
                }
                return tex3D(_VolumeTex, uvw).r * _DensityScale;
            }

            fixed4 frag(v2f i) : SV_Target
            {
                float3 ro = _WorldSpaceCameraPos;
                float3 rd = normalize(i.worldPos - ro);
                float2 t = intersectAabb(ro, rd, _BoundsMin, _BoundsMax);
                if (t.x > t.y || t.y < 0.0)
                {
                    return fixed4(0, 0, 0, 0);
                }

                float tStart = max(t.x, 0.0);
                float tEnd = t.y;
                float step = max(_StepSize, 1e-4);
                int steps = min((int)((tEnd - tStart) / step) + 1, (int)_MaxSteps);

                float accum = 0.0;
                float3 pos = ro + rd * tStart;
                for (int s = 0; s < steps; s++)
                {
                    float d = sampleDensity(pos);
                    accum += d * step;
                    pos += rd * step;
                    if (accum > 1.0)
                    {
                        break;
                    }
                }

                float alpha = saturate(accum);
                return fixed4(_Tint.rgb, alpha * _Tint.a);
            }
            ENDCG
        }
    }
    FallBack Off
}
