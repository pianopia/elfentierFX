import { compileGLSL } from './compiler.js';

/** Embed SPIRV-Cross functions in a UE Custom-node struct. Resource and varying
 * bindings are explicit: language conversion cannot infer the render pipeline. */
export function compiledMaterial(result, { name, bindings = {}, uniforms = {}, textures = {}, overrides = {}, extraFields = '', setup = '' } = {}) {
  if (result.stage !== 'frag') throw new Error('Material adapter requires a fragment entry point');
  const inputs = [], assignments = [];
  let hlsl = result.hlsl;
  hlsl = hlsl.replace(/cbuffer\s+\w+\s*:\s*register\(\w+\)\s*\{([\s\S]*?)\};/g, (_, body) => {
    return body.replace(/((?:row_major\s+)?\w+)\s+(\w+)\s*:\s*packoffset\([^)]*\);/g, (decl, type, field) => {
      // Unused built-in matrices remain in the block's reflection. No binding is needed.
      const escaped = field.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      if ((result.hlsl.match(new RegExp(`\\b${escaped}\\b`, 'g')) ?? []).length === 1) return '';
      const source = field.replace(/^_\d+_/, '');
      const binding = bindings[source];
      if (binding) {
        if (binding.input) inputs.push(binding.input);
        assignments.push(`fx.${field} = ${binding.code};`);
      } else {
        const value = uniforms[source];
        if (type === 'float' && Number.isFinite(value)) {
          inputs.push({ name: source, kind: 'scalar', value }); assignments.push(`fx.${field} = ${source};`);
        } else if (/^float[234]$/.test(type) && Array.isArray(value) && value.length === Number(type.at(-1)) && value.every(Number.isFinite)) {
          inputs.push({ name: source, kind: 'vector', value: [...value, ...Array(4 - value.length).fill(0)] });
          assignments.push(`fx.${field} = ${source}.${'xyzw'.slice(0, value.length)};`);
        } else throw new Error(`Missing material uniform binding: ${source} (${type})`);
      }
      return `${type} ${field};`;
    });
  });
  for (const texture of result.reflection.textures ?? []) {
    // A native function override can consume a renderer texture differently.
    if (textures[texture.name] === null) {
      hlsl = hlsl.replace(new RegExp(`^.*(?: ${texture.name}| _${texture.name}_sampler) : register\\([^)]*\\);\\r?\\n`, 'gm'), '');
      continue;
    }
    const path = textures[texture.name];
    if (typeof path !== 'string') throw new Error(`Missing native texture binding: ${texture.name}`);
    inputs.push({ name: texture.name, kind: 'texture', path });
    assignments.push(`fx.${texture.name} = ${texture.name};`, `fx._${texture.name}_sampler = ${texture.name}Sampler;`);
  }
  for (const input of result.reflection.inputs ?? []) {
    const binding = bindings[input.name];
    if (!binding) throw new Error(`Missing vertex varying binding: ${input.name}`);
    if (binding.input) inputs.push(binding.input);
    assignments.push(`fx.${input.name} = ${binding.code};`);
  }
  if ((result.reflection.outputs ?? []).length !== 1 || result.reflection.outputs[0].type !== 'vec4') throw new Error('Custom material requires a single vec4 fragment output');
  hlsl = hlsl.replace(/struct SPIRV_Cross_(?:Input|Output)\s*\{[\s\S]*?\};/g, '');
  const entry = hlsl.indexOf('SPIRV_Cross_Output main(');
  if (entry < 0) throw new Error('Unexpected SPIRV-Cross fragment entry format');
  hlsl = hlsl.slice(0, entry);
  hlsl = hlsl.replace(/ : register\([^)]*\)/g, '').replace(/^static /gm, '');
  for (const [functionName, body] of Object.entries(overrides)) {
    const start = hlsl.search(new RegExp(`\\b${functionName}\\([^)]*\\)\\s*\\{`));
    if (start < 0) throw new Error(`Missing native override function: ${functionName}`);
    const open = hlsl.indexOf('{', start); let depth = 1, end = open + 1;
    for (; end < hlsl.length && depth; end++) { if (hlsl[end] === '{') depth++; if (hlsl[end] === '}') depth--; }
    hlsl = hlsl.slice(0, open + 1) + '\n' + body + '\n' + hlsl.slice(end - 1);
  }
  const output = result.reflection.outputs[0].name;
  const code = `struct EFXCompiledShader {\n${extraFields}\n${hlsl}\n};\nEFXCompiledShader fx;\n${setup}\n${assignments.join('\n')}\nfx.frag_main();\nreturn fx.${output};`;
  const unique = new Map(inputs.map(input => [input.name, input]));
  return { name, kind: 'compiled_shader', output: 'float4', hlsl: code, inputs: [...unique.values()] };
}

export async function compileMaterial(source, options) {
  const result = await compileGLSL(source, { ...options, stage: 'frag' });
  return { result, recipe: compiledMaterial(result, options) };
}
