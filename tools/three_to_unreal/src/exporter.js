import { Mesh, MeshStandardMaterial, Matrix4, Color, Group, NormalBlending } from 'three';
import { GLTFExporter } from 'three/addons/exporters/GLTFExporter.js';
import { clone } from 'three/addons/utils/SkeletonUtils.js';
import { shaderRecipe } from './shaders.js';
import { encodeVolume, encodeSurfaceCache, waterRecipe } from './effects.js';

export { compileExpression } from './shaders.js';
export { encodeVolume, encodeSurfaceCache, waterRecipe, volumeFromTexture } from './effects.js';

export function safeName(name) {
  return String(name).replace(/[^A-Za-z0-9_]/g, '_').slice(0, 64) || 'asset';
}

/** Browser-safe: returns a Map<relative path, Uint8Array>. Does not mutate the scene.
 * strict=true prevents an apparently successful export when unsupported effects exist.
 */
export async function exportThreeBundle(scene, {
  name = 'ThreeScene', strict = true, animations = [], volumes = [], surfaceCaches = [], fps = 24,
} = {}) {
  if (!scene?.isObject3D) throw new Error('Expected a live THREE.Scene/Object3D');
  if (!Number.isFinite(fps) || fps <= 0 || fps > 240) throw new Error('Invalid fps');
  const files = new Map();
  const json = value => new TextEncoder().encode(JSON.stringify(value, null, 2) + '\n');
  const manifest = { format: 'elfentier_three_unreal_v1', version: 1, name: safeName(name),
    units: 'meters', up_axis: 'Y', frame_rate: fps,
    coordinates: { glb: 'UE Interchange converts glTF meters/Y-up to centimeters/Z-up',
      vdb: 'Converted by vdb_convert --ue-space: (x,y,z) -> 100*(x,z,y)',
      alembic: 'Blender exporter Y-up meters; importer uses explicit (1,-1,1) scale and (90,0,0) rotation' },
    materials: [], payloads: [], diagnostics: [] };
  const warn = (code, object, message) => manifest.diagnostics.push({ severity: 'warning', code, object, message });
  const error = (code, object, message) => manifest.diagnostics.push({ severity: 'error', code, object, message });
  const copied = clone(scene);
  const sourceObjects = []; scene.traverse(o => sourceObjects.push(o));
  let sourceIndex = 0;
  // Animation tracks may bind by UUID rather than name. Preserve source identity
  // inside the isolated clone, including cloned skeleton bones.
  copied.traverse(o => { o.uuid = sourceObjects[sourceIndex++].uuid; });
  const root = copied.isScene ? copied : new Group().add(copied);
  const replacements = new Map();
  const objects = []; root.traverse(o => objects.push(o));
  let instanceCount = 0;
  for (const obj of objects) {
    obj.userData = {};
    if (obj.isPoints || obj.isLine || obj.isSprite) error('UNSUPPORTED_PRIMITIVE', obj.name, 'Bake points/lines/sprites to meshes or use a Niagara adapter');
    if (obj.geometry?.isInstancedBufferGeometry && !obj.isInstancedMesh) error('GPU_ATTRIBUTES', obj.name, 'Custom instanced attributes require CPU surface snapshots');
    if (obj.isInstancedMesh) {
      instanceCount += obj.count;
      if (instanceCount > 10000) throw new Error('Instance expansion exceeds 10000 meshes; bake/merge first');
      if (obj.instanceColor) error('INSTANCE_COLOR', obj.name, 'Per-instance colors need material/vertex-color baking');
      if (obj.morphTexture) error('INSTANCE_MORPH', obj.name, 'Instance morph weights require per-instance baking');
      if (animations.some(clip => clip.tracks.some(track => track.name.startsWith(obj.uuid + '.') || (obj.name && track.name.startsWith(obj.name + '.'))))) error('INSTANCE_ANIMATION', obj.name, 'Animated instance containers require surface snapshots');
      obj.updateMatrix();
      for (let i = 0; i < obj.count; i++) {
        const item = new Mesh(obj.geometry, obj.material);
        item.name = `${obj.name || 'instance'}_${i}`;
        const matrix = new Matrix4(); obj.getMatrixAt(i, matrix);
        item.matrix.multiplyMatrices(obj.matrix, matrix);
        item.matrix.decompose(item.position, item.quaternion, item.scale);
        item.visible = obj.visible;
        obj.parent?.add(item);
        objects.push(item);
      }
      obj.parent?.remove(obj);
      continue;
    }
    if (!obj.material) continue;
    const wasArray = Array.isArray(obj.material);
    const materials = wasArray ? obj.material : [obj.material];
    if (wasArray && obj.geometry?.groups.some(group => !materials[group.materialIndex])) {
      throw new Error(`Mesh ${obj.name} has geometry groups without a matching material`);
    }
    obj.material = materials.map(material => {
      if (replacements.has(material)) return replacements.get(material);
      const id = `EFX_${String(replacements.size).padStart(4, '0')}_${safeName(material.name || material.type)}`;
      let recipe = { name: id, kind: 'gltf_pbr', sourceName: material.name, sourceType: material.type };
      let replacement;
      try {
        if (material.blending !== NormalBlending || material.wireframe || (material.side === 1 && !material.userData?.elfentierUE?.compiled) || material.clippingPlanes?.length) error('MATERIAL_RENDER_STATE', obj.name, 'Blending/wireframe/back-face/clipping behavior requires a native adapter');
        if (material.userData?.elfentierUE?.compiled) {
          recipe = { ...material.userData.elfentierUE.compiled, name: id };
          if (recipe.kind !== 'compiled_shader' || recipe.output !== 'float4' || typeof recipe.hlsl !== 'string') throw new Error('Invalid compiled material recipe');
          replacement = new MeshStandardMaterial({ color: 0xffffff });
        } else if (material.userData?.elfentierUE?.water) {
          recipe = waterRecipe(material.userData.elfentierUE.water, id);
          warn('WATER_APPROXIMATION', obj.name, recipe.note);
          replacement = new MeshStandardMaterial({ color: new Color().fromArray(recipe.color), roughness: recipe.roughness });
        } else if (material.isShaderMaterial || material.isRawShaderMaterial) {
          recipe = shaderRecipe(material, id);
          if (recipe.adapterExplicit) warn('EXPLICIT_SHADER_ADAPTER', obj.name, 'Adapter exports the supplied fragment expression only; source vertex shader is not reproduced');
          replacement = new MeshStandardMaterial({ color: 0xffffff });
          files.set(`shaders/${id}.glsl.json`, json({ vertex: material.vertexShader, fragment: material.fragmentShader }));
        } else {
          if (!material.isMeshStandardMaterial && !material.isMeshBasicMaterial) error('NON_PBR_MATERIAL', obj.name, `${material.type} requires a PBR or expression adapter`);
          if (material.onBeforeCompile.toString() !== MeshStandardMaterial.prototype.onBeforeCompile.toString()) error('SHADER_HOOK', obj.name, 'onBeforeCompile modifications are not portable; use an explicit adapter');
          for (const key of ['displacementMap', 'bumpMap', 'alphaMap', 'lightMap']) {
            if (material[key]) error('UNSUPPORTED_MAP', obj.name, `glTF does not preserve ${key}; bake geometry/normal/alpha/light data first`);
          }
          if (material.isMeshPhysicalMaterial) warn('PHYSICAL_MATERIAL', obj.name, 'Physical extensions are preserved in GLB; UE support varies. Verify transmission, IOR, sheen and iridescence in UE');
          replacement = material.clone();
        }
      } catch (e) {
        error('SHADER_UNSUPPORTED', obj.name, e.message);
        recipe = { name: id, kind: 'unsupported', reason: e.message };
        replacement = new MeshStandardMaterial({ color: 0xff00ff });
      }
      replacement.name = id;
      // Avoid serializing arbitrary userData/uniform references into glTF extras.
      replacement.userData = {};
      replacements.set(material, replacement);
      manifest.materials.push(recipe);
      return replacement;
    });
    if (!wasArray) obj.material = obj.material[0];
  }
  if (scene.environment || scene.background || scene.fog) warn('SCENE_LOOK', name, 'Environment, background, fog, postprocessing and tone mapping need UE lighting setup');
  const errors = manifest.diagnostics.filter(d => d.severity === 'error');
  if (strict && errors.length) {
    const exception = new Error(errors.map(d => `${d.code} [${d.object}]: ${d.message}`).join('\n'));
    exception.diagnostics = manifest.diagnostics; throw exception;
  }
  // Standard glTF stays in meters/Y-up; never rotate/scale it a second time.
  const glb = await new GLTFExporter().parseAsync(root, { binary: true, animations, onlyVisible: true, maxTextureSize: 4096 });
  files.set('scene.glb', new Uint8Array(glb));
  manifest.payloads.push({ format: 'gltf_glb', path: 'scene.glb' });
  volumes.forEach((volume, i) => {
    const id = `${String(i).padStart(3, '0')}_${safeName(volume.name || 'density')}`;
    const path = `volumes/${id}.evol`;
    files.set(path, encodeVolume(volume));
    manifest.payloads.push({ format: 'elfentier_volume_texture_v1', path, frame_count: volume.frames.length,
      vdb_prefix: `volumes/${id}/density`, status: 'requires_vdb_conversion' });
  });
  surfaceCaches.forEach((cache, i) => {
    const id = `${String(i).padStart(3, '0')}_${safeName(cache.name || 'surface')}`;
    const path = `surfaces/${id}.json`;
    const encoded = encodeSurfaceCache(cache);
    files.set(path, json(encoded));
    manifest.payloads.push({ format: encoded.format, path, frame_count: encoded.frames.length,
      alembic_path: `surfaces/${id}.abc`, status: 'requires_alembic_bake' });
  });
  manifest.payloads.forEach(p => { p.byte_len = files.get(p.path).byteLength; });
  files.set('manifest.json', json(manifest));
  files.set('conversion-report.json', json({ complete: !errors.length && !manifest.payloads.some(p => p.status), diagnostics: manifest.diagnostics,
    pending: manifest.payloads.filter(p => p.status).map(p => ({ path: p.path, status: p.status })) }));
  for (const material of replacements.values()) material.dispose();
  return { files, manifest };
}
