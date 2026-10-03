import { FloatType, RGBAFormat, ClampToEdgeWrapping, RepeatWrapping, MirroredRepeatWrapping, NearestFilter, LinearFilter } from 'three';

const identifier = name => typeof name === 'string' && /^[A-Za-z_]\w{0,63}$/.test(name);
const wrap = value => new Map([[ClampToEdgeWrapping,'clamp'],[RepeatWrapping,'repeat'],[MirroredRepeatWrapping,'mirror']]).get(value);
const filter = value => new Map([[NearestFilter,'nearest'],[LinearFilter,'linear']]).get(value);

/** Capture the public GPUComputationRenderer variables before init(). Run this
 * on a live browser object. JS update callbacks must supply explicit bindings. */
export function captureGPUComputation(gpu, { name = 'Fluid', bindings = {}, width, height } = {}) {
  if (!Array.isArray(gpu?.variables) || !gpu.variables.length) throw new Error('Expected GPUComputationRenderer variables');
  const first = gpu.variables[0].initialValueTexture?.image;
  width ??= first?.width; height ??= first?.height;
  const graph = { format: 'elfentier_gpu_graph_v1', name, width, height, semantics: 'simultaneous_previous_epoch', variables: [] };
  for (const variable of gpu.variables) {
    if (variable.renderTargets?.some(target => target.texture.type !== FloatType)) throw new Error('Native graph requires Float32 render targets; HalfFloat precision needs an explicit adapter');
    const texture = variable.initialValueTexture, image = texture?.image;
    if (texture?.type !== FloatType || texture?.format !== RGBAFormat || !(image?.data instanceof Float32Array) || image.width !== width || image.height !== height) throw new Error(`Initial texture must be matching RGBA Float32 CPU data: ${variable.name}`);
    const uniforms = {};
    const deps = (variable.dependencies ?? []).map(dep => dep.name);
    for (const [key, uniform] of Object.entries(variable.material.uniforms)) {
      if (deps.includes(key)) continue; // dependency textures populated by init()
      const value = uniform.value;
      const data = typeof value === 'number' ? value : value?.toArray?.();
      if (!(Number.isFinite(data) || (Array.isArray(data) && data.length >= 2 && data.length <= 4 && data.every(Number.isFinite)))) throw new Error(`Unsupported uniform ${variable.name}.${key}; declare/bake external resources explicitly`);
      uniforms[key] = { value: data, binding: bindings[`${variable.name}.${key}`] ?? 'constant' };
    }
    graph.variables.push({ name: variable.name, shader: variable.material.fragmentShader,
      defines: { ...variable.material.defines }, dependencies: deps, initial: Array.from(image.data), uniforms,
      wrapS: wrap(variable.wrapS ?? ClampToEdgeWrapping), wrapT: wrap(variable.wrapT ?? ClampToEdgeWrapping),
      filter: filter(variable.minFilter ?? NearestFilter) });
    if (variable.magFilter != null && filter(variable.magFilter) !== filter(variable.minFilter ?? NearestFilter)) throw new Error('Compute texture min/mag filters must match');
  }
  validateGraph(graph); return graph;
}

export function validateGraph(graph) {
  if (graph?.format !== 'elfentier_gpu_graph_v1' || graph.semantics !== 'simultaneous_previous_epoch') throw new Error('Unsupported GPU graph semantics');
  if (!identifier(graph.name) || !Number.isInteger(graph.width) || !Number.isInteger(graph.height) || graph.width < 1 || graph.height < 1 || graph.width > 4096 || graph.height > 4096) throw new Error('Invalid graph dimensions/name');
  if (!Array.isArray(graph.variables) || !graph.variables.length || graph.variables.length > 32 || graph.width * graph.height * graph.variables.length * 16 > 512 * 1024 * 1024) throw new Error('Graph exceeds resource limit');
  const names = new Set();
  for (const v of graph.variables) {
    if (!identifier(v.name) || names.has(v.name)) throw new Error('Invalid/duplicate variable name'); names.add(v.name);
    if (typeof v.shader !== 'string' || !v.shader.trim()) throw new Error('Missing shader');
    if (!Array.isArray(v.initial) || v.initial.length !== graph.width * graph.height * 4 || !v.initial.every(x => Number.isFinite(x) && Number.isFinite(Math.fround(x)))) throw new Error(`Invalid initial Float32 state: ${v.name}`);
    if (!['repeat','clamp','mirror'].includes(v.wrapS) || !['repeat','clamp','mirror'].includes(v.wrapT) || !['nearest','linear'].includes(v.filter)) throw new Error('Unsupported sampler');
    if (!Array.isArray(v.dependencies) || new Set(v.dependencies).size !== v.dependencies.length) throw new Error('Invalid dependencies');
    for (const [key, u] of Object.entries(v.uniforms ?? {})) {
      const values = Array.isArray(u.value) ? u.value : [u.value];
      if (!identifier(key) || !values.length || values.length > 4 || !values.every(x => Number.isFinite(x) && Number.isFinite(Math.fround(x))) || !['constant','time','delta','frame'].includes(u.binding) || (u.binding !== 'constant' && values.length !== 1)) throw new Error(`Unsupported uniform binding: ${key}`);
    }
  }
  for (const v of graph.variables) for (const dep of v.dependencies) if (!names.has(dep)) throw new Error(`Unknown dependency: ${dep}`);
  return graph;
}

/** Full compute shader conversion, preserving loops/functions/texture reads.
 * Fragment derivatives/discard/MRT are intentionally rejected by the compiler. */
export async function compileGraph(graph, options = {}) {
  validateGraph(graph);
  const { compileGLSL, rewriteIdentifiers } = await import('./compiler.js');
  const passes = [];
  for (const variable of graph.variables) {
    let source = variable.shader;
    // GPUComputationRenderer may already have prepended sampler declarations.
    const missing = variable.dependencies.filter(name => !new RegExp(`uniform\\s+sampler2D\\s+${name}\\s*;`).test(source));
    source = missing.map(name => `uniform sampler2D ${name};`).join('\n') + '\n' + source;
    source = rewriteIdentifiers(source, { main: 'efx_solve', gl_FragColor: 'efx_Value', gl_FragCoord: 'efx_Coord', texture2D: 'texture' });
    source = source.replace(/^\s*#version[^\n]*$/gm, '');
    const prefix = `layout(local_size_x=8,local_size_y=8,local_size_z=1) in;
      layout(rgba32f,binding=30) uniform writeonly image2D efx_Output;
      vec4 efx_Value; vec4 efx_Coord;
      #define resolution vec2(${graph.width}.0,${graph.height}.0)`;
    source = `${prefix}\n${source}\nvoid main(){
      if(any(greaterThanEqual(gl_GlobalInvocationID.xy,uvec2(${graph.width},${graph.height})))) return;
      efx_Coord=vec4(vec2(gl_GlobalInvocationID.xy)+vec2(.5),.5,1.);
      efx_Value=vec4(0.);efx_solve();imageStore(efx_Output,ivec2(gl_GlobalInvocationID.xy),efx_Value);}`;
    const defines = { ...variable.defines }; delete defines.resolution;
    const result = await compileGLSL(source, { ...options, stage: 'comp', defines });
    const resources = (result.reflection.textures ?? []).map(t => t.name);
    if (resources.some(name => !variable.dependencies.includes(name))) throw new Error(`Pass ${variable.name} has external textures requiring an explicit resource adapter`);
    if (result.reflection.ssbos?.length || (result.reflection.images ?? []).some(i => i.name !== 'efx_Output')) throw new Error('Extra storage/image resources require an explicit adapter');
    passes.push({ variable, result });
  }
  return { graph, passes };
}
