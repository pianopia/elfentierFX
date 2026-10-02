import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import * as THREE from 'three';
import { exportThreeBundle, compileExpression, encodeVolume, encodeSurfaceCache, waterRecipe, volumeFromTexture } from '../src/exporter.js';
import { writeBundle, bakeBundle } from '../src/node.js';
import { createScene } from '../examples/scene.mjs';

function readGLB(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  assert.equal(view.getUint32(0, true), 0x46546c67);
  assert.equal(view.getUint32(4, true), 2);
  assert.equal(view.getUint32(8, true), bytes.byteLength);
  return JSON.parse(new TextDecoder().decode(bytes.slice(20, 20 + view.getUint32(12, true))));
}
test('PBR, custom shader, water, volume and surface emit a coherent bundle without mutating scene', async () => {
  const data = createScene();
  const original = data.scene.children[2].material;
  const { files, manifest } = await exportThreeBundle(data.scene, data);
  const gltf = readGLB(files.get('scene.glb'));
  assert.equal(gltf.meshes.length, 3);
  assert.equal(gltf.materials.length, 3);
  assert.ok(gltf.extensionsUsed.includes('KHR_materials_clearcoat'));
  assert.equal(manifest.materials[1].kind, 'water_wpo');
  assert.match(manifest.materials[2].hlsl, /sin\(pulse\)/);
  assert.equal(data.scene.children[2].material, original);
  assert.equal(original.name, 'Pulse');
  assert.equal(manifest.payloads.length, 3);
  for (const payload of manifest.payloads) assert.equal(payload.byte_len, files.get(payload.path).byteLength);
});
test('GLB retains geometry groups, hierarchy, skin-independent transforms and meter units', async () => {
  const scene = new THREE.Scene(); const group = new THREE.Group(); group.position.set(3, 2, 1); scene.add(group);
  const mesh = new THREE.Mesh(new THREE.BoxGeometry(2, 4, 6), [new THREE.MeshStandardMaterial(), new THREE.MeshStandardMaterial()]);
  mesh.geometry.groups.forEach(group => { group.materialIndex %= 2; });
  group.add(mesh);
  const gltf = readGLB((await exportThreeBundle(scene)).files.get('scene.glb'));
  assert.deepEqual(gltf.nodes.find(n => n.matrix)?.matrix.slice(12, 15), [3, 2, 1]);
  assert.equal(gltf.meshes[0].primitives.length, 6);
  const accessor = gltf.accessors[gltf.meshes[0].primitives[0].attributes.POSITION];
  assert.deepEqual(accessor.min, [-1, -2, -3]); assert.deepEqual(accessor.max, [1, 2, 3]);
});
test('instance expansion preserves distinct transforms', async () => {
  const scene = new THREE.Scene();
  const mesh = new THREE.InstancedMesh(new THREE.BoxGeometry(), new THREE.MeshStandardMaterial(), 2);
  mesh.setMatrixAt(0, new THREE.Matrix4().makeTranslation(2, 0, 0));
  mesh.setMatrixAt(1, new THREE.Matrix4().makeTranslation(4, 0, 0));
  mesh.position.y = 1; scene.add(mesh);
  const gltf = readGLB((await exportThreeBundle(scene)).files.get('scene.glb'));
  assert.ok(gltf.nodes.some(n => JSON.stringify(n.matrix?.slice(12, 15)) === '[2,1,0]'));
  assert.ok(gltf.nodes.some(n => JSON.stringify(n.matrix?.slice(12, 15)) === '[4,1,0]'));
});
test('root InstancedMesh input is expanded and animation tracks retain UUID binding after cloning', async () => {
  const instance = new THREE.InstancedMesh(new THREE.BoxGeometry(), new THREE.MeshStandardMaterial(), 1);
  instance.setMatrixAt(0, new THREE.Matrix4().makeTranslation(5, 0, 0));
  const instanced = readGLB((await exportThreeBundle(instance)).files.get('scene.glb'));
  assert.ok(instanced.nodes.some(n => n.matrix?.[12] === 5));
  const mesh = new THREE.Mesh(new THREE.BoxGeometry(), new THREE.MeshStandardMaterial());
  const clip = new THREE.AnimationClip('Move', 1, [new THREE.VectorKeyframeTrack(`${mesh.uuid}.position`, [0, 1], [0, 0, 0, 1, 2, 3])]);
  const gltf = readGLB((await exportThreeBundle(mesh, { animations: [clip] })).files.get('scene.glb'));
  assert.equal(gltf.animations.length, 1);
  assert.equal(gltf.animations[0].channels[0].target.path, 'translation');
});
test('unsupported shader fails strict export and partial export stays visibly diagnostic', async () => {
  const scene = new THREE.Scene(); scene.add(new THREE.Mesh(new THREE.BoxGeometry(), new THREE.ShaderMaterial({ fragmentShader: 'void main() { discard; }' })));
  await assert.rejects(exportThreeBundle(scene), /single-assignment/);
  const result = await exportThreeBundle(scene, { strict: false });
  assert.equal(result.manifest.materials[0].kind, 'unsupported');
  assert.equal(JSON.parse(new TextDecoder().decode(result.files.get('conversion-report.json'))).complete, false);
});
test('onBeforeCompile modifications cannot silently disappear', async () => {
  const scene = new THREE.Scene(); const mat = new THREE.MeshStandardMaterial(); mat.onBeforeCompile = shader => { shader.fragmentShader += 'x'; };
  scene.add(new THREE.Mesh(new THREE.BoxGeometry(), mat));
  await assert.rejects(exportThreeBundle(scene), /SHADER_HOOK/);
});
test('GLSL expression semantics preserve floor-based negative modulo and function mappings', () => {
  assert.match(compileExpression('mod(-1.0, 2.0)'), /floor/);
  assert.match(compileExpression('mix(fract(1.5), 0.0, 0.5)'), /lerp\(frac/);
  assert.match(compileExpression('atan(1.0, 2.0)'), /atan2/);
  assert.throws(() => compileExpression('texture2D(map, uv)'), /Unbound|Unsupported/);
  assert.throws(() => compileExpression('foo + 1.0'), /Unbound/);
  assert.throws(() => compileExpression('1.0; discard;'), /token/);
});
test('expression compiler checks vector sizes, swizzles and scientific constants', () => {
  const bindings = { UV: { code: 'UV', size: 2 } };
  assert.match(compileExpression('vec4(UV, 1e-3, 1.0)', bindings), /float4\(UV, 1e-3, 1.0\)/);
  assert.match(compileExpression('vec2(UV)', bindings), /float2\(UV\)/);
  assert.throws(() => compileExpression('vec4(UV)', bindings), /component count/);
  assert.throws(() => compileExpression('UV.z', bindings), /dimensions/);
  assert.throws(() => compileExpression('vec3(1.0) + UV', bindings), /dimensions/);
  assert.throws(() => compileExpression('1e999'), /Non-finite/);
});
test('fragment expression adapter maps Time, UV and vector parameters into typed Custom inputs', async () => {
  const mat = new THREE.ShaderMaterial({ uniforms: { tint: { value: new THREE.Vector4(0.1, 0.2, 0.3, 0.4) } } });
  mat.userData.elfentierUE = { fragmentExpression: 'vec4(vUv, sin(t), tint.w)', bindings: { vUv: 'uv', t: 'time' } };
  const scene = new THREE.Scene(); scene.add(new THREE.Mesh(new THREE.BoxGeometry(), mat));
  const result = await exportThreeBundle(scene);
  assert.match(result.manifest.materials[0].hlsl, /float4\(UV, sin\(Time\)/);
  assert.deepEqual(result.manifest.materials[0].inputs.find(i => i.name === 'tint').value, [0.1, 0.2, 0.3, 0.4]);
  mat.userData.elfentierUE.fragmentExpression = 'sin(Time)';
  await assert.rejects(exportThreeBundle(scene), /return vec4/);
});
test('CPU scalar Data3DTexture supports float and normalized byte density', () => {
  const texture = new THREE.Data3DTexture(Uint8Array.of(0, 255), 2, 1, 1);
  texture.format = THREE.RedFormat;
  const volume = volumeFromTexture(texture, { boundsMin: [0, 0, 0], boundsMax: [1, 1, 1] });
  assert.deepEqual([...volume.frames[0]], [0, 1]);
  assert.deepEqual(volume.resolution, [2, 1, 1]);
  texture.format = THREE.RGBAFormat;
  assert.throws(() => volumeFromTexture(texture, {}), /scalar/);
});
test('volume header and x-fastest frames match the Rust EFVT interchange format', () => {
  const bytes = encodeVolume({ resolution: [2, 1, 1], boundsMin: [0, 1, 2], boundsMax: [2, 3, 4], frames: [[1, 2], [3, 4]] });
  const view = new DataView(bytes.buffer);
  assert.equal(new TextDecoder().decode(bytes.slice(0, 4)), 'EFVT');
  assert.equal(view.getUint32(20, true), 2); assert.equal(view.getFloat32(52 + 8, true), 3);
  assert.equal(bytes.length, 68);
});
test('malformed volume and surface inputs are rejected', () => {
  const volume = { resolution: [1, 1, 1], boundsMin: [0, 0, 0], boundsMax: [1, 1, 1], frames: [[1]] };
  assert.throws(() => encodeVolume({ ...volume, frames: [[NaN]] }), /Invalid/);
  assert.throws(() => encodeVolume({ ...volume, frames: [[-1]] }), /nonnegative/);
  assert.throws(() => encodeVolume({ ...volume, resolution: [0, 1, 1] }), /Resolution/);
  assert.throws(() => encodeSurfaceCache({ frames: [[0, 0, 0]], indices: [0, 1, 2] }), /indices/);
  assert.throws(() => waterRecipe({ waves: [{ direction: [0, 0], amplitude: 1, wavelength: 1, speed: 1 }] }, 'water'), /wave/);
});
test('writer refuses overwrites and path traversal', async () => {
  const root = await mkdtemp(join(tmpdir(), 'efx-three-'));
  await assert.rejects(writeBundle({ files: new Map([['../escape', new Uint8Array([1])]]) }, root), /Unsafe/);
  await writeFile(join(root, 'existing'), 'data');
  await assert.rejects(writeBundle({ files: new Map() }, root), /empty/);
});
test('pending native bakes remain pending and report is incomplete', async () => {
  const root = await mkdtemp(join(tmpdir(), 'efx-bake-'));
  const data = createScene(); await writeBundle(await exportThreeBundle(data.scene, data), root);
  await bakeBundle(root);
  const report = JSON.parse(await readFile(join(root, 'conversion-report.json'), 'utf8'));
  assert.equal(report.complete, false); assert.equal(report.pending.length, 2);
});
