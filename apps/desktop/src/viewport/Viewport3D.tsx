import { useCallback, useEffect, useRef, useState } from "react";
import { agentApi } from "../agent/api";
import type {
  NativePreviewImage,
  NativeViewportCamera,
  ViewportEnvironment,
  ViewportEnvironmentPreset,
  ViewportMesh,
} from "../types/graph";
import { saveViewportEnvironment } from "./environmentStorage";
import "./Viewport3D.css";

interface Viewport3DProps {
  mesh: ViewportMesh | null;
  nativePreview?: NativePreviewImage | null;
  nativeCamera?: NativeViewportCamera | null;
  initialError?: string | null;
  environment: ViewportEnvironment;
  onEnvironmentChange: (next: ViewportEnvironment) => void;
}

interface ViewportStats {
  particles: number;
  fluidFrame: number;
  backend: string;
}

function ViewportErrorPanel({ message }: { message: string }) {
  return (
    <div className="viewport3d-error">
      <p className="viewport3d-error-title">
        ネイティブ GPU ビューポートを起動できません
      </p>
      <p className="viewport3d-error-title-en">
        Native GPU viewport could not start
      </p>
      <pre className="viewport3d-error-detail">{message}</pre>
    </div>
  );
}

function decodePreviewRgba(preview: NativePreviewImage): Uint8ClampedArray {
  const expected = preview.width * preview.height * 4;
  if (!preview.rgba_base64) {
    throw new Error("Native preview is missing rgba_base64 payload");
  }
  const binary = atob(preview.rgba_base64);
  if (binary.length !== expected) {
    throw new Error(
      `Native preview RGBA length ${binary.length} does not match ${preview.width}x${preview.height} (expected ${expected} bytes)`,
    );
  }
  const rgba = new Uint8ClampedArray(expected);
  for (let i = 0; i < expected; i++) {
    rgba[i] = binary.charCodeAt(i);
  }
  return rgba;
}

function drawPreview(canvas: HTMLCanvasElement, preview: NativePreviewImage) {
  canvas.width = preview.width;
  canvas.height = preview.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("2D canvas context unavailable for native wgpu preview");
  }
  const rgba = decodePreviewRgba(preview);
  const imageData = new ImageData(preview.width, preview.height);
  imageData.data.set(rgba);
  ctx.putImageData(imageData, 0, 0);
}

function orbitCamera(
  camera: NativeViewportCamera,
  deltaAzimuth: number,
  deltaElevation: number,
): NativeViewportCamera {
  const dx = camera.eye[0] - camera.target[0];
  const dy = camera.eye[1] - camera.target[1];
  const dz = camera.eye[2] - camera.target[2];
  const radius = Math.max(Math.hypot(dx, dy, dz), 0.001);
  let azimuth = Math.atan2(dz, dx);
  let elevation = Math.asin(Math.max(-1, Math.min(1, dy / radius)));
  azimuth += deltaAzimuth;
  elevation = Math.max(-1.2, Math.min(1.2, elevation + deltaElevation));
  const cosEl = Math.cos(elevation);
  return {
    ...camera,
    eye: [
      camera.target[0] + radius * cosEl * Math.cos(azimuth),
      camera.target[1] + radius * Math.sin(elevation),
      camera.target[2] + radius * cosEl * Math.sin(azimuth),
    ],
  };
}

function fluidFrameCount(mesh: ViewportMesh): number {
  if (mesh.liquid && mesh.liquid.frame_count > 0) {
    return mesh.liquid.frame_count;
  }
  if (mesh.smoke && mesh.smoke.frame_count > 0) {
    return mesh.smoke.frame_count;
  }
  return 0;
}

function fluidFps(mesh: ViewportMesh): number {
  if (mesh.liquid) return mesh.liquid.fps || 12;
  if (mesh.smoke) return mesh.smoke.fps || 12;
  return 12;
}

function particleCountAt(mesh: ViewportMesh, frameIndex: number): number {
  if (mesh.liquid) {
    return (
      mesh.liquid.frames[frameIndex]?.particle_count ??
      mesh.liquid.frames[0]?.particle_count ??
      0
    );
  }
  if (mesh.smoke) {
    return (
      mesh.smoke.frames[frameIndex]?.particle_count ??
      mesh.smoke.frames[0]?.particle_count ??
      0
    );
  }
  return 0;
}

function fluidLabel(mesh: ViewportMesh | null): string {
  if (mesh?.liquid) return "liquid";
  if (mesh?.smoke) return "smoke";
  return "";
}

const PRESET_LABELS: Record<ViewportEnvironmentPreset, string> = {
  flat_gray: "Flat gray",
  studio_soft: "Studio soft",
  studio_contrast: "Studio contrast",
  custom: "Custom HDR",
};

export default function Viewport3D({
  mesh,
  nativePreview,
  nativeCamera,
  initialError = null,
  environment,
  onEnvironmentChange,
}: Viewport3DProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const meshRef = useRef<ViewportMesh | null>(null);
  const cameraRef = useRef<NativeViewportCamera | null>(null);
  const environmentRef = useRef(environment);
  const fluidFrameRef = useRef(0);
  const renderPendingRef = useRef(false);
  const dragRef = useRef<{ x: number; y: number } | null>(null);
  const [stats, setStats] = useState<ViewportStats>({
    particles: 0,
    fluidFrame: 0,
    backend: "wgpu",
  });
  const [viewportError, setViewportError] = useState<string | null>(initialError);

  const canRender = !!mesh && !viewportError;

  useEffect(() => {
    environmentRef.current = environment;
  }, [environment]);

  const reportRenderError = useCallback((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    console.error("Native wgpu viewport render failed:", message);
    setViewportError(message);
  }, []);

  const renderFrame = useCallback(
    async (frameIndex: number, camera: NativeViewportCamera) => {
      const currentMesh = meshRef.current;
      const container = containerRef.current;
      if (!currentMesh || !container || renderPendingRef.current) return;
      renderPendingRef.current = true;
      try {
        const width = Math.max(320, Math.min(container.clientWidth, 1280));
        const height = Math.max(240, Math.min(container.clientHeight, 960));
        const preview = await agentApi.renderNativeViewport(
          currentMesh,
          frameIndex,
          width,
          height,
          camera,
          environmentRef.current,
        );
        if (canvasRef.current) {
          drawPreview(canvasRef.current, preview);
        }
        setStats((prev) => ({
          ...prev,
          backend: preview.backend,
          fluidFrame: frameIndex,
          particles: particleCountAt(currentMesh, frameIndex),
        }));
      } catch (error) {
        reportRenderError(error);
      } finally {
        renderPendingRef.current = false;
      }
    },
    [reportRenderError],
  );

  const updateEnvironment = useCallback(
    (patch: Partial<ViewportEnvironment>) => {
      const next = { ...environmentRef.current, ...patch };
      environmentRef.current = next;
      saveViewportEnvironment(next);
      onEnvironmentChange(next);
    },
    [onEnvironmentChange],
  );

  useEffect(() => {
    if (!canRender || !mesh || !cameraRef.current) return;
    void renderFrame(fluidFrameRef.current, cameraRef.current);
  }, [environment, canRender, mesh, renderFrame]);

  useEffect(() => {
    setViewportError(initialError);
  }, [initialError, mesh]);

  useEffect(() => {
    meshRef.current = mesh;
    fluidFrameRef.current = 0;
    if (nativeCamera) {
      cameraRef.current = nativeCamera;
    }
    if (!canRender || !mesh) return;

    if (nativePreview && canvasRef.current) {
      try {
        drawPreview(canvasRef.current, nativePreview);
        setStats((prev) => ({
          ...prev,
          backend: nativePreview.backend,
          particles: particleCountAt(mesh, 0),
        }));
      } catch (error) {
        reportRenderError(error);
      }
    } else if (cameraRef.current) {
      void renderFrame(0, cameraRef.current);
    }
  }, [mesh, nativePreview, nativeCamera, canRender, renderFrame, reportRenderError]);

  useEffect(() => {
    if (!canRender || !mesh) return undefined;
    const frameCount = fluidFrameCount(mesh);
    if (frameCount <= 1) return undefined;
    const fps = fluidFps(mesh);
    const interval = window.setInterval(() => {
      fluidFrameRef.current = (fluidFrameRef.current + 1) % frameCount;
      const camera = cameraRef.current;
      if (camera) {
        void renderFrame(fluidFrameRef.current, camera);
      }
    }, 1000 / fps);
    return () => window.clearInterval(interval);
  }, [mesh, canRender, renderFrame]);

  useEffect(() => {
    if (!canRender) return undefined;
    const canvas = canvasRef.current;
    if (!canvas) return undefined;

    const onPointerDown = (event: PointerEvent) => {
      dragRef.current = { x: event.clientX, y: event.clientY };
      canvas.setPointerCapture(event.pointerId);
    };

    const onPointerMove = (event: PointerEvent) => {
      if (!dragRef.current || !cameraRef.current) return;
      const dx = event.clientX - dragRef.current.x;
      const dy = event.clientY - dragRef.current.y;
      dragRef.current = { x: event.clientX, y: event.clientY };
      cameraRef.current = orbitCamera(cameraRef.current, dx * 0.008, dy * 0.008);
      void renderFrame(fluidFrameRef.current, cameraRef.current);
    };

    const onPointerUp = (event: PointerEvent) => {
      dragRef.current = null;
      canvas.releasePointerCapture(event.pointerId);
    };

    canvas.addEventListener("pointerdown", onPointerDown);
    canvas.addEventListener("pointermove", onPointerMove);
    canvas.addEventListener("pointerup", onPointerUp);
    canvas.addEventListener("pointerleave", onPointerUp);
    return () => {
      canvas.removeEventListener("pointerdown", onPointerDown);
      canvas.removeEventListener("pointermove", onPointerMove);
      canvas.removeEventListener("pointerup", onPointerUp);
      canvas.removeEventListener("pointerleave", onPointerUp);
    };
  }, [canRender, renderFrame]);

  const label = fluidLabel(mesh);

  const handlePickHdr = async () => {
    try {
      const path = await agentApi.pickHdrFile();
      if (path) {
        updateEnvironment({ preset: "custom", hdr_path: path, enabled: true });
      }
    } catch (error) {
      console.error("HDR file picker failed:", error);
    }
  };

  return (
    <div className="viewport3d">
      <div className="viewport3d-chrome">
        <span className="viewport3d-title">3D View</span>
        {viewportError ? (
          <span className="viewport3d-stat viewport3d-stat-error">wgpu error</span>
        ) : (
          <>
            <span className="viewport3d-stat">
              native {stats.backend} · drag to orbit
            </span>
            <span className="viewport3d-stat">
              {stats.particles > 0 && label && `${stats.particles} ${label} · `}
              {stats.fluidFrame > 0 && `f${stats.fluidFrame} · `}
              wgpu preview
            </span>
          </>
        )}
      </div>
      <div className="viewport3d-env">
        <label className="viewport3d-env-field">
          <span>Env</span>
          <select
            value={environment.preset}
            onChange={(event) =>
              updateEnvironment({
                preset: event.target.value as ViewportEnvironmentPreset,
                enabled: event.target.value !== "flat_gray",
              })
            }
          >
            {Object.entries(PRESET_LABELS).map(([value, labelText]) => (
              <option key={value} value={value}>{labelText}</option>
            ))}
          </select>
        </label>
        <label className="viewport3d-env-field">
          <span>Intensity</span>
          <input
            type="range"
            min={0.2}
            max={3}
            step={0.05}
            value={environment.intensity}
            onChange={(event) =>
              updateEnvironment({ intensity: Number(event.target.value) })
            }
          />
        </label>
        <label className="viewport3d-env-field">
          <span>Yaw</span>
          <input
            type="range"
            min={-180}
            max={180}
            step={1}
            value={environment.rotation_yaw_deg}
            onChange={(event) =>
              updateEnvironment({ rotation_yaw_deg: Number(event.target.value) })
            }
          />
        </label>
        <label className="viewport3d-env-field">
          <span>Diffuse blur</span>
          <input
            type="range"
            min={0}
            max={1}
            step={0.05}
            value={environment.diffuse_blur}
            onChange={(event) =>
              updateEnvironment({ diffuse_blur: Number(event.target.value) })
            }
          />
        </label>
        {environment.preset === "custom" && (
          <button type="button" className="viewport3d-env-button" onClick={handlePickHdr}>
            Pick HDR
          </button>
        )}
      </div>
      <div className="viewport3d-canvas viewport3d-native" ref={containerRef}>
        {viewportError ? (
          <ViewportErrorPanel message={viewportError} />
        ) : (
          <>
            <canvas ref={canvasRef} className="viewport3d-native-canvas" />
            {!mesh && (
              <div className="viewport3d-empty">Cook to preview via native wgpu</div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
