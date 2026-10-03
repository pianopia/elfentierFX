import { mkdir, writeFile, readFile, readdir, stat } from 'node:fs/promises';
import { resolve, dirname, relative, isAbsolute } from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { exportThreeBundle } from './exporter.js';

// GLTFExporter uses FileReader even for texture-free binary output in Node.
if (!globalThis.FileReader) {
  globalThis.FileReader = class {
    readAsArrayBuffer(blob) { blob.arrayBuffer().then(result => { this.result = result; this.onloadend?.(); }).catch(e => this.onerror?.(e)); }
    readAsDataURL(blob) { blob.arrayBuffer().then(result => { this.result = `data:${blob.type};base64,${Buffer.from(result).toString('base64')}`; this.onloadend?.(); }).catch(e => this.onerror?.(e)); }
  };
}

function inside(root, path) {
  if (typeof path !== 'string' || isAbsolute(path) || path.includes('\\')) throw new Error('Payload path must be relative POSIX path');
  const full = resolve(root, path);
  const rel = relative(root, full);
  if (rel.startsWith('..') || isAbsolute(rel)) throw new Error(`Unsafe payload path: ${path}`);
  return full;
}
async function run(command, args) {
  await new Promise((accept, reject) => {
    const child = spawn(command, args, { stdio: 'inherit', shell: false, windowsHide: true });
    child.on('error', reject);
    child.on('exit', code => code === 0 ? accept() : reject(new Error(`${command} exited with ${code}`)));
  });
}

export async function writeBundle(bundle, out) {
  const root = resolve(out);
  try { if ((await readdir(root)).length) throw new Error('Output directory must be empty (existing files will not be overwritten)'); }
  catch (e) { if (e.code !== 'ENOENT') throw e; }
  for (const [path, bytes] of bundle.files) {
    const full = inside(root, path);
    await mkdir(dirname(full), { recursive: true });
    await writeFile(full, bytes, { flag: 'wx' });
  }
  return root;
}

/** Optional native baking; failure leaves source data, but never marks it ready. */
export async function bakeBundle(root, { blender, vdbConverter } = {}) {
  const manifestPath = resolve(root, 'manifest.json');
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  if (manifest.format !== 'elfentier_three_unreal_v1' || manifest.version !== 1) throw new Error('Unsupported bundle');
  const baker = fileURLToPath(new URL('../blender/bake_surface_cache.py', import.meta.url));
  for (const payload of [...manifest.payloads]) {
    if (payload.status === 'baked') continue;
    if (payload.format === 'elfentier_surface_cache_v1' && blender) {
      const output = inside(root, payload.alembic_path);
      await run(blender, ['--background', '--factory-startup', '--python-exit-code', '1', '--python', baker, '--', inside(root, payload.path), output]);
      const size = (await stat(output)).size;
      if (!size) throw new Error('Alembic baker produced an empty file');
      manifest.payloads.push({ format: 'alembic_geometry_cache', path: payload.alembic_path, byte_len: size, frame_count: payload.frame_count });
      payload.status = 'baked';
    }
    if (payload.format === 'elfentier_volume_texture_v1' && vdbConverter) {
      const prefix = inside(root, payload.vdb_prefix);
      await mkdir(dirname(prefix), { recursive: true });
      await run(vdbConverter, ['to-vdb', `${prefix}.vdb`, inside(root, payload.path), '--sequence', '--ue-space']);
      const paths = [];
      for (let i = 0; i < payload.frame_count; i++) {
        const path = `${payload.vdb_prefix}_${String(i).padStart(4, '0')}.vdb`;
        if (!(await stat(inside(root, path))).size) throw new Error('Missing VDB output');
        paths.push(path);
      }
      manifest.payloads.push({ format: 'openvdb_sequence', path: paths[0], paths, frame_count: paths.length, units: 'centimeters', up_axis: 'Z' });
      payload.status = 'baked';
    }
    // Persist after each successful subprocess so partial runs remain inspectable.
    await writeFile(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
  }
  const report = JSON.parse(await readFile(resolve(root, 'conversion-report.json'), 'utf8'));
  report.pending = manifest.payloads.filter(p => p.status && p.status !== 'baked').map(p => ({ path: p.path, status: p.status }));
  report.complete = report.pending.length === 0 && !manifest.diagnostics.some(d => d.severity === 'error');
  await writeFile(resolve(root, 'conversion-report.json'), JSON.stringify(report, null, 2) + '\n');
  return manifest;
}

export async function exportToDirectory(scene, out, options = {}) {
  // Node has no DOM image/canvas. Browser exportThreeBundle handles textured models.
  scene.traverse(obj => {
    for (const material of (Array.isArray(obj.material) ? obj.material : [obj.material]).filter(Boolean)) {
      if (Object.values(material).some(v => v?.isTexture)) throw new Error('Textured scenes require exportThreeBundle in a browser with loaded images/canvas');
    }
  });
  const bundle = await exportThreeBundle(scene, options);
  const root = await writeBundle(bundle, out);
  const manifest = await bakeBundle(root, options);
  return { root, manifest };
}
