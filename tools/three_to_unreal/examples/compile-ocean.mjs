// Read-only integration test against the actual local project's shader exports.
// Usage: node examples/compile-ocean.mjs <three-ocean-beach> <empty-output>
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { compileGLSL, writeCompiledShader } from '../src/compiler.js';

const root = resolve(process.argv[2]), out = resolve(process.argv[3]);
const shaders = await import(pathToFileURL(join(root, 'src/shaders.js')).href);
// Match the source project's Three.js version, including packing functions.
const { ShaderChunk } = await import(pathToFileURL(join(root, 'node_modules/three/build/three.module.js')).href);
const prefix = `uniform mat4 modelMatrix; uniform mat4 modelViewMatrix;
  uniform mat4 projectionMatrix; uniform mat4 viewMatrix; uniform vec3 cameraPosition;`;
const main = await readFile(join(root, 'src/main.js'), 'utf8');
const report = { source: root, shaders: [], scope: 'Full shader language compilation; resource wiring and JS application orchestration are separate.' };
for (const name of ['sky', 'terrain', 'water', 'grass']) {
  for (const stage of ['vert', 'frag']) {
    const key = name + (stage === 'vert' ? 'Vertex' : 'Fragment');
    let source = shaders[key];
    if (name === 'grass') {
      const body = main.match(new RegExp(`const ${key} = common \\+ /\\* glsl \\*/\x60([\\s\\S]*?)\x60;`));
      if (!body) throw new Error(`Source layout changed: cannot extract ${key}`);
      source = shaders.common + body[1];
    }
    const originalHash = createHash('sha256').update(source).digest('hex');
    // UE applies tonemapping and output color space once in its own post process.
    source = source.replace(/#include <(?:tonemapping_fragment|colorspace_fragment)>/g, '');
    const attributes = stage === 'vert' ? 'attribute vec3 position; attribute vec3 normal; attribute vec2 uv;' + (name === 'grass' ? 'attribute mat4 instanceMatrix;' : '') : '';
    const result = await compileGLSL(source, { stage, chunks: ShaderChunk, prefix: prefix + attributes });
    await writeCompiledShader(result, join(out, key));
    report.shaders.push({ name: key, originalHash, stage, compiled: true, inputs: result.reflection.inputs, textures: result.reflection.textures ?? [] });
  }
}
await mkdir(out, { recursive: true });
await writeFile(join(out, 'ocean-report.json'), JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
console.log(`OCEAN_GLSL_COMPILE_PASS ${report.shaders.length} shaders: ${out}`);
