#!/usr/bin/env node
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { exportToDirectory, bakeBundle } from './node.js';
import { readFile } from 'node:fs/promises';
import { compileGLSL, writeCompiledShader } from './compiler.js';
import { exportNativeGraph } from './native-graph.js';

const usage = 'three-to-unreal <trusted-scene.mjs> --out <empty-directory> [--blender <exe>] [--vdb-converter <exe>] [--allow-partial]\nthree-to-unreal bake <existing-bundle-directory> [--blender <exe>] [--vdb-converter <exe>]\nthree-to-unreal shader <source.glsl> --stage vert|frag|comp --out <directory> [--glslang <exe>] [--spirv-cross <exe>] [--vulkan]\nthree-to-unreal gpu <captured-graph.json> --out <empty-plugin-directory> [--glslang <exe>] [--spirv-cross <exe>]';
try {
  const args = process.argv.slice(2);
  if (args.includes('--help') || !args.length) { console.log(usage); process.exit(args.length ? 0 : 1); }
  if (args[0] === 'gpu') {
    args.shift(); const input=args.shift(); const config={};let output;
    while(args.length){const key=args.shift();if(!['--out','--glslang','--spirv-cross'].includes(key)||!args.length)throw new Error(usage);const value=args.shift();if(key==='--out')output=value;else config[key==='--glslang'?'glslang':'spirvCross']=value;}
    if(!input||!output)throw new Error(usage);
    const graph=JSON.parse(await readFile(resolve(input),'utf8'));
    const result=await exportNativeGraph(graph,output,config);
    console.log(`Native GPU plugin: ${result.root}; component ${result.component}. Build and GPU validation are still required.`);
    process.exit(0);
  }
  if (args[0] === 'shader') {
    args.shift(); const input = args.shift(); const config = {}; let output;
    while (args.length) {
      const key = args.shift();
      if (key === '--vulkan') { config.webgl = false; continue; }
      if (!['--stage', '--out', '--glslang', '--spirv-cross'].includes(key) || !args.length) throw new Error(usage);
      const value = args.shift();
      if (key === '--out') output = value;
      else config[{ '--stage': 'stage', '--glslang': 'glslang', '--spirv-cross': 'spirvCross' }[key]] = value;
    }
    if (!input || !output || !config.stage) throw new Error(usage);
    const result = await compileGLSL(await readFile(resolve(input), 'utf8'), config);
    console.log(`Compiled ${result.stage}: ${await writeCompiledShader(result, output)} (native resource wiring is separate)`);
    process.exit(0);
  }
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
