import test from 'node:test';
import assert from 'node:assert/strict';
import { DataTexture, FloatType, RGBAFormat } from 'three';
import { captureGPUComputation, validateGraph, compileGraph } from '../src/gpu-graph.js';

export function fixture() {
  const texture = new DataTexture(new Float32Array([1,2,3,4,5,6,7,8]),2,1,RGBAFormat,FloatType);
  const velocity = { name:'velocity',initialValueTexture:texture,material:{fragmentShader:'uniform float dt; void main(){vec2 uv=gl_FragCoord.xy/resolution;gl_FragColor=texture2D(velocity,uv)+vec4(dt);}',uniforms:{dt:{value:.1}}} };
  const density = { name:'density',initialValueTexture:texture,material:{fragmentShader:'void main(){vec2 uv=gl_FragCoord.xy/resolution;gl_FragColor=texture2D(velocity,uv)*2.0+texture2D(density,uv);}',uniforms:{}} };
  velocity.dependencies=[velocity]; density.dependencies=[velocity,density];
  return captureGPUComputation({variables:[velocity,density]}, {bindings:{'velocity.dt':'delta'}});
}
test('capture preserves all passes, dependencies, initial states and epoch semantics', () => {
  const graph=fixture(); assert.equal(graph.semantics,'simultaneous_previous_epoch');
  assert.deepEqual(graph.variables[1].dependencies,['velocity','density']);
  assert.equal(graph.variables[0].uniforms.dt.binding,'delta');
  assert.deepEqual(graph.variables[0].initial,[1,2,3,4,5,6,7,8]);
  graph.variables[1].dependencies.push('missing'); assert.throws(()=>validateGraph(graph),/Unknown/);
});
test('reject lossy/infinite state, dynamic vectors and unsupported filters', () => {
  const graph=fixture(); graph.variables[0].initial[0]=1e100; assert.throws(()=>validateGraph(graph),/Float32/);
  graph.variables[0].initial[0]=1; graph.variables[0].uniforms.dt={value:[1,2],binding:'delta'}; assert.throws(()=>validateGraph(graph),/binding/);
});
test('real compiler emits compute entrypoints for the whole graph', {skip:!process.env.GLSLANG_VALIDATOR||!process.env.SPIRV_CROSS}, async()=>{
  const {passes}=await compileGraph(fixture());
  assert.equal(passes.length,2);
  assert.deepEqual(passes[0].result.reflection.entryPoints[0].workgroup_size,[8,8,1]);
  assert.match(passes[1].result.hlsl,/RWTexture2D<float4>/);
  assert.equal(passes[1].result.reflection.textures.length,2);
});
