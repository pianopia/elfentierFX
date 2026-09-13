import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { ViewportMesh, ViewportSmoke } from "../types/graph";
import "./Viewport3D.css";

type RendererKind = "webgpu" | "webgl";

interface ThreeViewportFallbackProps {
  mesh: ViewportMesh | null;
}

interface ViewportStats {
  fps: number;
  triangles: number;
  instances: number;
  particles: number;
  smokeFrame: number;
  drawCalls: number;
  renderer: RendererKind;
}

function fitCameraToSmoke(
  camera: THREE.PerspectiveCamera,
  controls: OrbitControls,
  smoke: ViewportSmoke,
) {
  const box = new THREE.Box3(
    new THREE.Vector3(...smoke.bounds_min),
    new THREE.Vector3(...smoke.bounds_max),
  );
  if (box.isEmpty()) return;
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3());
  const maxDim = Math.max(size.x, size.y, size.z, 6);
  const dist = maxDim * 1.8;
  camera.position.set(center.x + dist * 0.65, center.y + dist * 0.5, center.z + dist * 0.75);
  controls.target.copy(center);
  controls.update();
}

function fitCameraToContent(
  camera: THREE.PerspectiveCamera,
  controls: OrbitControls,
  mesh: ViewportMesh,
) {
  if (mesh.smoke && mesh.vertex_count === 0) {
    fitCameraToSmoke(camera, controls, mesh.smoke);
    return;
  }

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

  if (mesh.smoke) {
    box.expandByPoint(new THREE.Vector3(...mesh.smoke.bounds_min));
    box.expandByPoint(new THREE.Vector3(...mesh.smoke.bounds_max));
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

function applySmokeFrame(
  smokeMesh: THREE.InstancedMesh,
  smoke: ViewportSmoke,
  frameIndex: number,
) {
  const frame = smoke.frames[frameIndex];
  if (!frame) return 0;

  const dummy = new THREE.Object3D();
  const count = frame.particle_count;
  for (let i = 0; i < count; i++) {
    const px = frame.positions[i * 3];
    const py = frame.positions[i * 3 + 1];
    const pz = frame.positions[i * 3 + 2];
    const size = frame.sizes[i] ?? 0.2;
    const opacity = frame.opacities[i] ?? 0.5;
    dummy.position.set(px, py, pz);
    dummy.scale.setScalar(size);
    dummy.updateMatrix();
    smokeMesh.setMatrixAt(i, dummy.matrix);
    smokeMesh.setColorAt(i, new THREE.Color(0.85, 0.88, 0.92).multiplyScalar(0.55 + opacity * 0.55));
  }
  smokeMesh.count = count;
  smokeMesh.instanceMatrix.needsUpdate = true;
  if (smokeMesh.instanceColor) smokeMesh.instanceColor.needsUpdate = true;
  return count;
}

function applyMeshToScene(
  scene: THREE.Scene,
  mesh: ViewportMesh,
  instancedRef: React.MutableRefObject<THREE.InstancedMesh | null>,
  smokeRef: React.MutableRefObject<THREE.InstancedMesh | null>,
  camera: THREE.PerspectiveCamera,
  controls: OrbitControls,
) {
  if (instancedRef.current) {
    scene.remove(instancedRef.current);
    instancedRef.current.geometry.dispose();
    (instancedRef.current.material as THREE.Material).dispose();
    instancedRef.current = null;
  }
  if (smokeRef.current) {
    scene.remove(smokeRef.current);
    smokeRef.current.geometry.dispose();
    (smokeRef.current.material as THREE.Material).dispose();
    smokeRef.current = null;
  }

  if (mesh.vertex_count > 0 && mesh.positions.length > 0) {
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
  }

  if (mesh.smoke && mesh.smoke.frames.length > 0) {
    const maxParticles = Math.max(
      ...mesh.smoke.frames.map((f) => f.particle_count),
      64,
    );
    const particleGeo = new THREE.SphereGeometry(0.5, 6, 6);
    const particleMat = new THREE.MeshStandardMaterial({
      color: 0xd8e2ef,
      transparent: true,
      opacity: 0.35,
      depthWrite: false,
      roughness: 1.0,
      metalness: 0.0,
    });
    const smokeInstanced = new THREE.InstancedMesh(particleGeo, particleMat, maxParticles);
    applySmokeFrame(smokeInstanced, mesh.smoke, 0);
    scene.add(smokeInstanced);
    smokeRef.current = smokeInstanced;
  }

  fitCameraToContent(camera, controls, mesh);
}

/** Three.js viewport fallback when native wgpu preview is unavailable. */
export default function ThreeViewportFallback({ mesh }: ThreeViewportFallbackProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<THREE.Scene | null>(null);
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
  const controlsRef = useRef<OrbitControls | null>(null);
  const instancedRef = useRef<THREE.InstancedMesh | null>(null);
  const smokeRef = useRef<THREE.InstancedMesh | null>(null);
  const readyRef = useRef(false);
  const meshRef = useRef<ViewportMesh | null>(null);
  const smokeFrameRef = useRef(0);
  const smokeClockRef = useRef(0);
  const [stats, setStats] = useState<ViewportStats>({
    fps: 0,
    triangles: 0,
    instances: 0,
    particles: 0,
    smokeFrame: 0,
    drawCalls: 0,
    renderer: "webgl",
  });

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let disposed = false;
    let animationId = 0;
    let glRenderer: THREE.WebGLRenderer | null = null;
    let gpuRenderer: THREE.WebGLRenderer | null = null;
    let rendererKind: RendererKind = "webgl";

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0c0e12);
    scene.fog = new THREE.FogExp2(0x0c0e12, 0.018);
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
      if (meshRef.current && controlsRef.current) {
        applyMeshToScene(
          scene,
          meshRef.current,
          instancedRef,
          smokeRef,
          camera,
          controlsRef.current,
        );
        const m = meshRef.current;
        setStats((prev) => ({
          ...prev,
          triangles: m.triangle_count * m.instance_count,
          instances: m.instance_count,
          particles: m.smoke?.frames[0]?.particle_count ?? 0,
          drawCalls: (instancedRef.current ? 1 : 0) + (smokeRef.current ? 1 : 0),
        }));
      }

      let frames = 0;
      let lastFps = performance.now();

      const tick = (now: number) => {
        if (disposed) return;
        animationId = requestAnimationFrame(tick);
        controlsRef.current?.update();

        const currentMesh = meshRef.current;
        const smokeData = currentMesh?.smoke;
        if (smokeData && smokeRef.current && smokeData.frame_count > 1) {
          smokeClockRef.current += 1 / 60;
          const frameDuration = 1 / smokeData.fps;
          if (smokeClockRef.current >= frameDuration) {
            smokeClockRef.current = 0;
            smokeFrameRef.current = (smokeFrameRef.current + 1) % smokeData.frame_count;
            const particles = applySmokeFrame(
              smokeRef.current,
              smokeData,
              smokeFrameRef.current,
            );
            setStats((prev) => ({
              ...prev,
              particles,
              smokeFrame: smokeFrameRef.current,
            }));
          }
        }

        (gpuRenderer ?? glRenderer)?.render(scene, camera);

        frames += 1;
        if (now - lastFps >= 500) {
          const fps = Math.round((frames * 1000) / (now - lastFps));
          frames = 0;
          lastFps = now;
          setStats((prev) => ({
            ...prev,
            fps,
            drawCalls:
              (instancedRef.current ? 1 : 0) + (smokeRef.current ? 1 : 0),
          }));
        }
      };
      requestAnimationFrame(tick);
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
      if (smokeRef.current) {
        scene.remove(smokeRef.current);
        smokeRef.current.geometry.dispose();
        (smokeRef.current.material as THREE.Material).dispose();
        smokeRef.current = null;
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
  }, []);

  useEffect(() => {
    meshRef.current = mesh;
    smokeFrameRef.current = 0;
    smokeClockRef.current = 0;
    const scene = sceneRef.current;
    const camera = cameraRef.current;
    const controls = controlsRef.current;
    if (!mesh || !scene || !camera || !controls || !readyRef.current) return;

    applyMeshToScene(scene, mesh, instancedRef, smokeRef, camera, controls);
    setStats((prev) => ({
      ...prev,
      triangles: mesh.triangle_count * mesh.instance_count,
      instances: mesh.instance_count,
      particles: mesh.smoke?.frames[0]?.particle_count ?? 0,
      smokeFrame: 0,
      drawCalls:
        (instancedRef.current ? 1 : 0) + (smokeRef.current ? 1 : 0),
    }));
  }, [mesh]);

  return (
    <div className="viewport3d">
      <div className="viewport3d-chrome">
        <span className="viewport3d-title">3D View</span>
        <span className="viewport3d-stat">
          fallback {stats.renderer === "webgpu" ? "WebGPU" : "WebGL"} · {stats.fps} fps
        </span>
        <span className="viewport3d-stat">
          {stats.instances > 0 && `${stats.instances} inst · `}
          {stats.particles > 0 && `${stats.particles} smoke · `}
          {stats.smokeFrame > 0 && `f${stats.smokeFrame} · `}
          {(stats.triangles / 1000).toFixed(1)}k tris · {stats.drawCalls} draw
        </span>
      </div>
      <div className="viewport3d-canvas" ref={containerRef} />
      {!mesh && (
        <div className="viewport3d-empty">Cook to preview city geometry or smoke</div>
      )}
    </div>
  );
}
