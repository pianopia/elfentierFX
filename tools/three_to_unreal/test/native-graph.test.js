import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fluidGraph, referenceFluid } from '../examples/stable-fluid.mjs';
import { exportNativeGraph } from '../src/native-graph.js';
import { compileMaterial } from '../src/material-compiler.js';

test('whole solver reference transports dye and produces pressure feedback',()=>{
  const graph=fluidGraph();const state=referenceFluid(graph,.35,6);
  assert.equal(Object.keys(state).length,5);
  assert.ok(state.pressure.some((v,i)=>i%4===0&&Math.abs(v)>.01));
  assert.notDeepEqual(state.dye,graph.variables[4].initial);
  assert.notDeepEqual(state.velocity,graph.variables[3].initial);
});
test('native generation preserves all epochs/resources and refuses overwrite',{skip:!process.env.GLSLANG_VALIDATOR||!process.env.SPIRV_CROSS},async()=>{
  const temp=await mkdtemp(join(tmpdir(),'elfentier-native-test-'));
  try{
    const root=join(temp,'plugin');const result=await exportNativeGraph(fluidGraph(),root);
    assert.equal(result.compiled.passes.length,5);
    const cpp=await readFile(join(root,'Source/ElfentierGpuStableFluid/Private/ElfentierGpuStableFluid.cpp'),'utf8');
    assert.equal((cpp.match(/IMPLEMENT_GLOBAL_SHADER/g)??[]).length,5);
    // UE's real parameter parser rejects a leading underscore, even when HLSL
    // compiles. Both the C++ metadata and shader names must use the same alias.
    assert.doesNotMatch(cpp,/SHADER_PARAMETER[^\n]*,\s*_/);
    const usf=await readFile(join(root,'Shaders/Pass0.usf'),'utf8');
    assert.match(usf,/EfxUniform_dt/);
    assert.match(usf,/EfxTexture_velocity/);
    assert.match(usf,/EfxSampler_velocity/);
    const epochStart=cpp.indexOf('for(int32 Iteration=0;');const epochEnd=cpp.indexOf('Current=MoveTemp(Next)',epochStart);
    assert.equal((cpp.slice(epochStart,epochEnd).match(/FComputeShaderUtils::AddPass/g)??[]).length,5);
    assert.equal((await readFile(join(root,'Resources/velocity.bin'))).length,8*4*4*4);
    await assert.rejects(exportNativeGraph(fluidGraph(),root),/empty/);
  }finally{await rm(temp,{recursive:true,force:true});}
});
test('compiled material binds full functions/textures and rejects missing resources',{skip:!process.env.GLSLANG_VALIDATOR||!process.env.SPIRV_CROSS},async()=>{
  const shader='uniform sampler2D grains;uniform float t;varying vec2 uv;float noiseSum(vec2 p){float n=0.;for(int i=0;i<3;i++)n+=texture2D(grains,p*float(i+1)).x;return n;}void main(){gl_FragColor=vec4(noiseSum(uv)*t);}';
  const options={name:'Test',uniforms:{t:.25},textures:{grains:'textures/grains.tga'},bindings:{uv:{code:'UV.xy',input:{name:'UV',kind:'uv'}}}};
  const {recipe}=await compileMaterial(shader,options);
  assert.match(recipe.hlsl,/struct EFXCompiledShader/);assert.match(recipe.hlsl,/noiseSum/);
  assert.ok(recipe.inputs.some(i=>i.kind==='texture'));
  await assert.rejects(compileMaterial(shader,{...options,textures:{}}),/texture binding/);
});
