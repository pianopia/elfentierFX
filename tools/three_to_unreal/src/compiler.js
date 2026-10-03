import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { tmpdir } from 'node:os';
import { ShaderChunk } from 'three';

async function invoke(exe, args) {
  return new Promise((accept, reject) => {
    const process = spawn(exe, args, { shell: false, windowsHide: true });
    let stdout = '', stderr = '';
    process.stdout.on('data', data => { stdout += data; });
    process.stderr.on('data', data => { stderr += data; });
    process.on('error', reject);
    process.on('close', code => code === 0 ? accept(stdout) : reject(new Error(`${exe} failed (${code})\n${stdout}${stderr}`)));
  });
}

/** Resolve exactly the ShaderChunk dictionary supplied by the source project's
 * Three.js version. Renderer-dependent chunks still need the renderer prefix. */
export function expandIncludes(source, chunks = ShaderChunk, stack = []) {
  return source.replace(/^[ \t]*#include\s+<([\w]+)>/gm, (_, name) => {
    if (stack.includes(name)) throw new Error(`Recursive shader include: ${name}`);
    if (!Object.hasOwn(chunks, name)) throw new Error(`Unknown shader include: ${name}`);
    return expandIncludes(chunks[name], chunks, [...stack, name]);
  });
}

// Rewrite identifiers only, preserving comments and directives. The language
// compiler, rather than this compatibility pass, validates types and syntax.
export function rewriteIdentifiers(source, names) {
  return source.replace(/\/\*[\s\S]*?\*\/|\/\/[^\n]*|\b[A-Za-z_]\w*\b/g, token => names[token] ?? token);
}

export function prepareGLSL(source, { stage = 'frag', chunks, prefix = '', defines = {}, webgl = true } = {}) {
  if (!['vert', 'frag', 'comp'].includes(stage)) throw new Error('stage must be vert, frag or comp');
  if (typeof source !== 'string' || !source.trim() || source.length > 4 * 1024 * 1024) throw new Error('Invalid shader source');
  if (!webgl) return source; // Already Vulkan GLSL: leave its version/layouts intact.
  let text = expandIncludes(prefix + '\n' + source, chunks);
  text = text.replace(/^\s*#version[^\n]*$/gm, '').replace(/^\s*precision\s+\w+\s+\w+\s*;/gm, '');
  text = text.replace(/^\s*#extension\s+GL_(?:OES_standard_derivatives|EXT_shader_texture_lod)\s*:[^\n]*$/gm, '');
  const names = { texture2D: 'texture', textureCube: 'texture', texture2DLodEXT: 'textureLod', textureCubeLodEXT: 'textureLod', texture2DProj: 'textureProj', lowp: '', mediump: '', highp: '' };
  if (stage === 'vert') Object.assign(names, { attribute: 'in', varying: 'out' });
  if (stage === 'frag') Object.assign(names, { varying: 'in', gl_FragColor: 'efx_FragColor' });
  // Fully assembled WebGLRenderer source already supplies compatibility macros.
  // Preserve their definitions and uses, rather than producing '#define in in'.
  for (const name of Object.keys(names)) if (new RegExp(`^[ \\t]*#define\\s+${name}\\b`, 'm').test(text)) delete names[name];
  const fragColor = stage === 'frag' && Object.hasOwn(names, 'gl_FragColor') && /\bgl_FragColor\b/.test(text);
  text = rewriteIdentifiers(text, names);
  const macros = Object.entries(defines).map(([key, value]) => {
    if (!/^[A-Za-z_]\w*$/.test(key) || /[\r\n]/.test(String(value))) throw new Error('Invalid shader define');
    return `#define ${key} ${value === true ? 1 : value}`;
  }).join('\n');
  return `#version 450\n${macros}\n${fragColor ? 'layout(location=0) out vec4 efx_FragColor;' : ''}\n${text}`;
}

/** Full GLSL compilation through official compilers. No execution of source JS,
 * no shell interpolation, and no successful result on either compiler failure. */
export async function compileGLSL(source, options = {}) {
  const { stage = 'frag', glslang = process.env.GLSLANG_VALIDATOR || 'glslangValidator', spirvCross = process.env.SPIRV_CROSS || 'spirv-cross' } = options;
  const glsl = prepareGLSL(source, { ...options, stage });
  const temp = await mkdtemp(join(tmpdir(), 'elfentier-shader-'));
  try {
    const input = join(temp, `shader.${stage}`), binary = join(temp, 'shader.spv');
    await writeFile(input, glsl);
    const diagnostics = await invoke(glslang, ['-V', '-R', '--auto-map-bindings', '--auto-map-locations', '--set-default-uniform-block', 'EFXUniforms', '0', '31', '-S', stage, '-o', binary, input]);
    const hlsl = await invoke(spirvCross, [binary, '--hlsl', '--shader-model', '50']);
    const reflection = JSON.parse(await invoke(spirvCross, [binary, '--reflect']));
    const spirv = await readFile(binary);
    if (spirv.length < 20 || spirv.readUInt32LE(0) !== 0x07230203) throw new Error('Compiler produced invalid SPIR-V');
    return { glsl, spirv, hlsl, reflection, diagnostics, stage };
  } finally { await rm(temp, { recursive: true, force: true }); }
}

export async function writeCompiledShader(result, out) {
  const root = resolve(out);
  await mkdir(root, { recursive: true });
  // Exclusive creation intentionally refuses to replace an existing shader.
  for (const [path, value] of Object.entries({ 'source.glsl': result.glsl, 'shader.spv': result.spirv, 'shader.hlsl': result.hlsl,
    'reflection.json': JSON.stringify(result.reflection, null, 2) + '\n', 'compilation.json': JSON.stringify({ stage: result.stage, languageCompiled: true, nativePipelineReady: false, diagnostics: result.diagnostics }, null, 2) + '\n' })) {
    await writeFile(join(root, path), value, { flag: 'wx' });
  }
  return root;
}
