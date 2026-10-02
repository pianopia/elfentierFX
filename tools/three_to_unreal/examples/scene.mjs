import * as THREE from 'three';

export function createScene() {
  const scene = new THREE.Scene();
  const physical = new THREE.MeshPhysicalMaterial({ color: 0x99bbcc, metalness: 0.7, roughness: 0.23, clearcoat: 0.8 });
  physical.name = 'PolishedMetal';
  const sphere = new THREE.Mesh(new THREE.SphereGeometry(0.5, 24, 16), physical);
  sphere.name = 'MetalSphere'; sphere.position.set(0, 1, 0); scene.add(sphere);
  const water = new THREE.MeshStandardMaterial();
  water.name = 'Water';
  water.userData.elfentierUE = { water: { waves: [
    { direction: [1, 0.3], amplitude: 0.08, wavelength: 2.5, speed: 1.3 },
    { direction: [-0.4, 1], amplitude: 0.04, wavelength: 1.2, speed: 1.8 },
  ] } };
  const plane = new THREE.Mesh(new THREE.PlaneGeometry(4, 4, 32, 32), water);
  plane.name = 'WaterSurface'; plane.rotation.x = -Math.PI / 2; scene.add(plane);
  const shader = new THREE.ShaderMaterial({
    vertexShader: 'void main() { gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }',
    fragmentShader: 'uniform float pulse; varying vec2 vUv; void main() { gl_FragColor = vec4(vec3(0.5 + 0.5 * sin(pulse)), 1.0); }',
    uniforms: { pulse: { value: 1.2 } },
  });
  shader.name = 'Pulse';
  const cube = new THREE.Mesh(new THREE.BoxGeometry(0.5, 0.5, 0.5), shader);
  cube.name = 'ShaderCube'; cube.position.set(1, 0.5, 0); scene.add(cube);
  const n = 8;
  const density = time => Float32Array.from({ length: n ** 3 }, (_, i) => {
    const x = i % n, y = Math.floor(i / n) % n, z = Math.floor(i / (n * n));
    return Math.exp(-((x - 3.5) ** 2 + (y - 3 - time) ** 2 + (z - 3.5) ** 2) / 5);
  });
  const surfaceFrames = [0, 1, 2, 3].map(f => [0, 0, 0, 1, 0.1 * f, 0, 0, 0.05 * f, 2, 1, 0.15 * f, 2]);
  return { scene, name: 'ThreeNativeDemo',
    volumes: [{ name: 'Smoke', resolution: [n, n, n], boundsMin: [-1, 0, -1], boundsMax: [1, 2, 1], frames: [density(0), density(0.5)] }],
    surfaceCaches: [{ name: 'LiquidSurface', fps: 24, indices: [0, 2, 1, 1, 2, 3], frames: surfaceFrames }] };
}
