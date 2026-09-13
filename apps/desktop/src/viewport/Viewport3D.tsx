import { useCallback, useEffect, useRef, useState } from "react";
import { agentApi } from "../agent/api";
import type {
  NativePreviewImage,
  NativeViewportCamera,
  ViewportMesh,
} from "../types/graph";
import ThreeViewportFallback from "./ThreeViewportFallback";
import "./Viewport3D.css";

interface Viewport3DProps {
  mesh: ViewportMesh | null;
  nativePreview?: NativePreviewImage | null;
  nativeCamera?: NativeViewportCamera | null;
  useNativeViewport?: boolean;
}

interface ViewportStats {
  fps: number;
  particles: number;
  smokeFrame: number;
  backend: string;
}

function drawPreview(canvas: HTMLCanvasElement, preview: NativePreviewImage) {
  canvas.width = preview.width;
  canvas.height = preview.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const imageData = new ImageData(preview.width, preview.height);
  imageData.data.set(preview.rgba);
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

export default function Viewport3D({
  mesh,
  nativePreview,
  nativeCamera,
  useNativeViewport = false,
}: Viewport3DProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const meshRef = useRef<ViewportMesh | null>(null);
  const cameraRef = useRef<NativeViewportCamera | null>(null);
  const smokeFrameRef = useRef(0);
  const renderPendingRef = useRef(false);
  const dragRef = useRef<{ x: number; y: number } | null>(null);
  const [stats, setStats] = useState<ViewportStats>({
    fps: 0,
    particles: 0,
    smokeFrame: 0,
    backend: "wgpu",
  });
  const [renderError, setRenderError] = useState(false);

  const nativeActive = useNativeViewport && !renderError && !!mesh;

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
        );
        if (canvasRef.current) {
          drawPreview(canvasRef.current, preview);
        }
        setStats((prev) => ({
          ...prev,
          backend: preview.backend,
          smokeFrame: frameIndex,
          particles:
            currentMesh.smoke?.frames[frameIndex]?.particle_count ??
            currentMesh.smoke?.frames[0]?.particle_count ??
            0,
        }));
      } catch {
        setRenderError(true);
      } finally {
        renderPendingRef.current = false;
      }
    },
    [],
  );

  useEffect(() => {
    meshRef.current = mesh;
    smokeFrameRef.current = 0;
    if (nativeCamera) {
      cameraRef.current = nativeCamera;
    }
    if (!nativeActive || !mesh) return;

    if (nativePreview && canvasRef.current) {
      drawPreview(canvasRef.current, nativePreview);
      setStats((prev) => ({
        ...prev,
        backend: nativePreview.backend,
        particles: mesh.smoke?.frames[0]?.particle_count ?? 0,
      }));
    } else if (cameraRef.current) {
      void renderFrame(0, cameraRef.current);
    }
  }, [mesh, nativePreview, nativeCamera, nativeActive, renderFrame]);

  useEffect(() => {
    if (!nativeActive || !mesh?.smoke || mesh.smoke.frame_count <= 1) return undefined;
    const fps = mesh.smoke.fps || 12;
    const interval = window.setInterval(() => {
      smokeFrameRef.current = (smokeFrameRef.current + 1) % mesh.smoke!.frame_count;
      const camera = cameraRef.current;
      if (camera) {
        void renderFrame(smokeFrameRef.current, camera);
      }
    }, 1000 / fps);
    return () => window.clearInterval(interval);
  }, [mesh, nativeActive, renderFrame]);

  useEffect(() => {
    if (!nativeActive) return undefined;
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
      void renderFrame(smokeFrameRef.current, cameraRef.current);
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
  }, [nativeActive, renderFrame]);

  if (!nativeActive) {
    return <ThreeViewportFallback mesh={mesh} />;
  }

  return (
    <div className="viewport3d">
      <div className="viewport3d-chrome">
        <span className="viewport3d-title">3D View</span>
        <span className="viewport3d-stat">
          native {stats.backend} · drag to orbit
        </span>
        <span className="viewport3d-stat">
          {stats.particles > 0 && `${stats.particles} smoke · `}
          {stats.smokeFrame > 0 && `f${stats.smokeFrame} · `}
          wgpu preview
        </span>
      </div>
      <div className="viewport3d-canvas viewport3d-native" ref={containerRef}>
        <canvas ref={canvasRef} className="viewport3d-native-canvas" />
        {!mesh && (
          <div className="viewport3d-empty">Cook to preview via native wgpu</div>
        )}
      </div>
    </div>
  );
}
