import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { SmokePreviewImage, ViewportCook, ViewportMesh } from "../types/graph";
import "./Viewport3D.css";

type RendererKind = "webgpu" | "webgl" | "wgpu-smoke";

interface Viewport3DProps {
  viewport: ViewportCook | null;
  smokePreview: SmokePreviewImage | null;
}

interface ViewportStats {
  fps: number;
  triangles: number;
  instances: number;
  drawCalls: number;
  renderer: RendererKind;
}

function fitCameraToContent(
  camera: THREE.PerspectiveCamera,
  controls: OrbitControls,
  mesh: ViewportMesh,
) {
  const box = new THREE.Box3();
  const matrix = new THREE.Matrix4();
  const vec = new THREE.Vector3();
  const positions = mesh.positions;

  for (let i = 0; i < mesh.instance_count; i++) {
    matrix.fromArray(mesh.instance_matrices.slice(i * 16, i * 16 + 16));
    for (let v = 0; v < positions.length; v += 3) {
      vec.set(positions[v], positions[v + 1], positions[v + 2]).applyMatrix4(matrix);
      box.expandByPoint(vec);
    }
  }

  if (box.isEmpty()) return;

  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3());
  const maxDim = Math.max(size.x, size.y, size.z, 8);
  const dist = maxDim * 1.6;
  camera.position.set(center.x + dist * 0.7, center.y + dist * 0.55, center.z + dist * 0.85);
  controls.target.copy(center);
  controls.update();
}

function fitCameraToSmokeBounds(
  camera: THREE.PerspectiveCamera,
  controls: OrbitControls,
  boundsMin: [number, number, number],
  boundsMax: [number, number, number],
) {
  const center = new THREE.Vector3(
    (boundsMin[0] + boundsMax[0]) * 0.5,
    (boundsMin[1] + boundsMax[1]) * 0.5,
    (boundsMin[2] + boundsMax[2]) * 0.5,
  );
  const size = new THREE.Vector3(
    boundsMax[0] - boundsMin[0],
    boundsMax[1] - boundsMin[1],
    boundsMax[2] - boundsMin[2],
  );
  const maxDim = Math.max(size.x, size.y, size.z, 4);
  const dist = maxDim * 2.2;
  camera.position.set(center.x + dist * 0.55, center.y + dist * 0.45, center.z + dist * 0.75);
  controls.target.copy(center);
  controls.update();
}

function applyMeshToScene(
  scene: THREE.Scene,
  mesh: ViewportMesh,
  instancedRef: React.MutableRefObject<THREE.InstancedMesh | null>,
  camera: THREE.PerspectiveCamera,
  controls: OrbitControls,
) {
  if (instancedRef.current) {
    scene.remove(instancedRef.current);
    instancedRef.current.geometry.dispose();
    (instancedRef.current.material as THREE.Material).dispose();
    instancedRef.current = null;
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(mesh.positions, 3));
  geometry.setIndex(mesh.indices);
  geometry.computeVertexNormals();

  const material = new THREE.MeshStandardMaterial({
    color: 0x8fa4c4,
    roughness: 0.62,
    metalness: 0.12,
  });

  const count = mesh.instance_count;
  const instanced = new THREE.InstancedMesh(geometry, material, count);
  const matrix = new THREE.Matrix4();
  for (let i = 0; i < count; i++) {
    matrix.fromArray(mesh.instance_matrices.slice(i * 16, i * 16 + 16));
    instanced.setMatrixAt(i, matrix);
  }
  instanced.instanceMatrix.needsUpdate = true;
  scene.add(instanced);
  instancedRef.current = instanced;
  fitCameraToContent(camera, controls, mesh);
}

function drawSmokePreview(canvas: HTMLCanvasElement, preview: SmokePreviewImage) {
  canvas.width = preview.width;
  canvas.height = preview.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const imageData = new ImageData(preview.width, preview.height);
  imageData.data.set(preview.rgba);
  ctx.putImageData(imageData, 0, 0);
}

export default function Viewport3D({ viewport, smokePreview }: Viewport3DProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const smokeCanvasRef = useRef<HTMLCanvasElement>(null);
  const sceneRef = useRef<THREE.Scene | null>(null);
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
  const controlsRef = useRef<OrbitControls | null>(null);
  const instancedRef = useRef<THREE.InstancedMesh | null>(null);
  const readyRef = useRef(false);
  const viewportRef = useRef<ViewportCook | null>(null);
  const [stats, setStats] = useState<ViewportStats>({
    fps: 0,
    triangles: 0,
    instances: 0,
    drawCalls: 0,
    renderer: "webgl",
  });

  const isSmoke = viewport?.output_kind === "smoke";

  useEffect(() => {
    if (!isSmoke || !smokePreview || !smokeCanvasRef.current) return;
    drawSmokePreview(smokeCanvasRef.current, smokePreview);
    setStats((prev) => ({
      ...prev,
      renderer: "wgpu-smoke",
      drawCalls: 1,
      instances: viewport?.smoke?.density.length ?? 0,
      triangles: 0,
    }));
  }, [isSmoke, smokePreview, viewport]);

  useEffect(() => {
    if (isSmoke) return;

    const container = containerRef.current;
    if (!container) return;

    let disposed = false;
    let animationId = 0;
    let glRenderer: THREE.WebGLRenderer | null = null;
    let gpuRenderer: THREE.WebGLRenderer | null = null;
    let rendererKind: RendererKind = "webgl";

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0c0e12);
    scene.fog = new THREE.Fog(0x0c0e12, 80, 220);
    sceneRef.current = scene;

    const camera = new THREE.PerspectiveCamera(50, 1, 0.1, 500);
    camera.position.set(28, 22, 36);
    cameraRef.current = camera;

    scene.add(new THREE.AmbientLight(0x4a5568, 0.55));
    const key = new THREE.DirectionalLight(0xd8e6ff, 1.1);
    key.position.set(30, 50, 20);
    scene.add(key);
    const fill = new THREE.DirectionalLight(0x6ea8ff, 0.35);
    fill.position.set(-20, 15, -10);
    scene.add(fill);

    const grid = new THREE.GridHelper(120, 60, 0x2a3344, 0x1a1f28);
    grid.position.y = -0.01;
    scene.add(grid);

    const ground = new THREE.Mesh(
      new THREE.PlaneGeometry(120, 120),
      new THREE.MeshStandardMaterial({
        color: 0x10141a,
        roughness: 0.95,
        metalness: 0.05,
      }),
    );
    ground.rotation.x = -Math.PI / 2;
    ground.position.y = -0.02;
    scene.add(ground);

    const controls = new OrbitControls(camera, container);
    controls.enableDamping = true;
    controls.dampingFactor = 0.06;
    controls.target.set(24, 4, 0);
    controlsRef.current = controls;

    const resize = () => {
      const w = container.clientWidth;
      const h = container.clientHeight;
      if (w === 0 || h === 0) return;
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      const active = gpuRenderer ?? glRenderer;
      active?.setSize(w, h);
    };

    const mountCanvas = (canvas: HTMLCanvasElement) => {
      canvas.style.display = "block";
      canvas.style.width = "100%";
      canvas.style.height = "100%";
      container.appendChild(canvas);
      controls.dispose();
      const nextControls = new OrbitControls(camera, canvas);
      nextControls.enableDamping = true;
      nextControls.dampingFactor = 0.06;
      nextControls.target.copy(controls.target);
      controlsRef.current = nextControls;
    };

    const setup = async () => {
      try {
        const { WebGPURenderer } = await import("three/webgpu");
        const gpu = new WebGPURenderer({ antialias: true });
        await gpu.init();
        gpu.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        mountCanvas(gpu.domElement);
        gpuRenderer = gpu as unknown as THREE.WebGLRenderer;
        rendererKind = "webgpu";
      } catch {
        glRenderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
        glRenderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        glRenderer.outputColorSpace = THREE.SRGBColorSpace;
        mountCanvas(glRenderer.domElement);
      }

      setStats((prev) => ({ ...prev, renderer: rendererKind }));
      resize();
      readyRef.current = true;
      const mesh = viewportRef.current?.mesh;
      if (mesh && controlsRef.current) {
        applyMeshToScene(scene, mesh, instancedRef, camera, controlsRef.current);
        setStats((prev) => ({
          ...prev,
          triangles: mesh.triangle_count * mesh.instance_count,
          instances: mesh.instance_count,
          drawCalls: 1,
        }));
      }

      let frames = 0;
      let lastFps = performance.now();

      const tick = () => {
        if (disposed) return;
        animationId = requestAnimationFrame(tick);
        controlsRef.current?.update();
        (gpuRenderer ?? glRenderer)?.render(scene, camera);

        frames += 1;
        const now = performance.now();
        if (now - lastFps >= 500) {
          const fps = Math.round((frames * 1000) / (now - lastFps));
          frames = 0;
          lastFps = now;
          setStats((prev) => ({
            ...prev,
            fps,
            drawCalls: instancedRef.current ? 1 : 0,
          }));
        }
      };
      tick();
    };

    setup();

    const observer = new ResizeObserver(resize);
    observer.observe(container);

    return () => {
      disposed = true;
      cancelAnimationFrame(animationId);
      observer.disconnect();
      controlsRef.current?.dispose();
      if (instancedRef.current) {
        scene.remove(instancedRef.current);
        instancedRef.current.geometry.dispose();
        (instancedRef.current.material as THREE.Material).dispose();
        instancedRef.current = null;
      }
      grid.geometry.dispose();
      (grid.material as THREE.Material).dispose();
      ground.geometry.dispose();
      (ground.material as THREE.Material).dispose();
      gpuRenderer?.dispose();
      glRenderer?.dispose();
      sceneRef.current = null;
      readyRef.current = false;
      while (container.firstChild) {
        container.removeChild(container.firstChild);
      }
    };
  }, [isSmoke]);

  useEffect(() => {
    viewportRef.current = viewport;
    if (isSmoke) {
      if (viewport?.smoke && cameraRef.current && controlsRef.current) {
        fitCameraToSmokeBounds(
          cameraRef.current,
          controlsRef.current,
          viewport.smoke.bounds_min,
          viewport.smoke.bounds_max,
        );
      }
      return;
    }

    const mesh = viewport?.mesh;
    const scene = sceneRef.current;
    const camera = cameraRef.current;
    const controls = controlsRef.current;
    if (!mesh || !scene || !camera || !controls || !readyRef.current) return;

    applyMeshToScene(scene, mesh, instancedRef, camera, controls);
    setStats((prev) => ({
      ...prev,
      triangles: mesh.triangle_count * mesh.instance_count,
      instances: mesh.instance_count,
      drawCalls: 1,
    }));
  }, [viewport, isSmoke]);

  const rendererLabel =
    stats.renderer === "wgpu-smoke"
      ? "wgpu"
      : stats.renderer === "webgpu"
        ? "Three WebGPU (fallback mesh)"
        : "WebGL";

  return (
    <div className="viewport3d">
      <div className="viewport3d-chrome">
        <span className="viewport3d-title">3D View</span>
        <span className="viewport3d-stat">
          {rendererLabel} · {stats.fps} fps
        </span>
        {isSmoke ? (
          <span className="viewport3d-stat">
            {viewport?.smoke?.nx ?? 0}×{viewport?.smoke?.ny ?? 0}×{viewport?.smoke?.nz ?? 0} grid · wgpu raymarch
          </span>
        ) : (
          <span className="viewport3d-stat">
            {stats.instances} inst · {(stats.triangles / 1000).toFixed(1)}k tris · {stats.drawCalls} draw
          </span>
        )}
      </div>
      {isSmoke ? (
        <div className="viewport3d-canvas viewport3d-smoke">
          <canvas ref={smokeCanvasRef} className="viewport3d-smoke-canvas" />
          {!smokePreview && (
            <div className="viewport3d-empty">Cook to preview smoke via native wgpu</div>
          )}
        </div>
      ) : (
        <div className="viewport3d-canvas" ref={containerRef}>
          {!viewport?.mesh && (
            <div className="viewport3d-empty">Cook to preview city geometry</div>
          )}
        </div>
      )}
    </div>
  );
}
