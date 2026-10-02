#!/usr/bin/env node
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { exportToDirectory, bakeBundle } from './node.js';

const usage = 'three-to-unreal <trusted-scene.mjs> --out <empty-directory> [--blender <exe>] [--vdb-converter <exe>] [--allow-partial]\nthree-to-unreal bake <existing-bundle-directory> [--blender <exe>] [--vdb-converter <exe>]';
try {
  const args = process.argv.slice(2);
  if (args.includes('--help') || !args.length) { console.log(usage); process.exit(args.length ? 0 : 1); }
  const bakeOnly = args[0] === 'bake';
  if (bakeOnly) args.shift();
  const input = args.shift();
  if (!input) throw new Error(usage);
  const options = {};
  let out;
  while (args.length) {
    const flag = args.shift();
    if (flag === '--allow-partial') { options.strict = false; continue; }
    if (!['--out', '--blender', '--vdb-converter'].includes(flag) || !args.length || args[0].startsWith('--')) throw new Error(`Invalid option: ${flag}\n${usage}`);
    const value = args.shift();
    if (flag === '--out') out = value;
    else options[flag === '--blender' ? 'blender' : 'vdbConverter'] = value;
  }
  if (bakeOnly) {
    if (out || options.strict === false) throw new Error('bake does not accept --out / --allow-partial');
    await bakeBundle(resolve(input), options);
    console.log(`Baked: ${resolve(input)} (see conversion-report.json)`);
    process.exit(0);
  }
  if (!out) throw new Error(usage);
  // Explicitly trusted code input, never auto-fetch or eval arbitrary source URLs.
  const source = await import(pathToFileURL(resolve(input)).href);
  const sceneData = source.createScene ? await source.createScene() : source.default;
  const data = sceneData?.isObject3D ? { scene: sceneData } : sceneData;
  if (!data?.scene) throw new Error('Module must export createScene() or default {scene, volumes?, surfaceCaches?, animations?}');
  const result = await exportToDirectory(data.scene, out, { ...data, ...options });
  console.log(`Exported: ${result.root}`);
  const pending = result.manifest.payloads.filter(p => p.status && p.status !== 'baked');
  if (pending.length) console.log(`Pending native bakes: ${pending.map(p => p.path).join(', ')} (see conversion-report.json)`);
  if (result.manifest.diagnostics.some(d => d.severity === 'error')) process.exitCode = 2;
} catch (error) { console.error(error.message); process.exitCode = 1; }
