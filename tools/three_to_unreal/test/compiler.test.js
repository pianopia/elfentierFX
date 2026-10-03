import test from 'node:test';
import assert from 'node:assert/strict';
import { prepareGLSL, rewriteIdentifiers, expandIncludes, compileGLSL } from '../src/compiler.js';

test('WebGL compatibility preserves comments, resolves chunks and stages', () => {
  const text = prepareGLSL('varying vec2 uv; // varying texture2D\nvoid main(){gl_FragColor=texture2D(tex,uv);}', { prefix: 'uniform sampler2D tex;', defines: { SAMPLES: 4 } });
  assert.match(text, /in vec2 uv/);
  assert.match(text, /\/\/ varying texture2D/);
  assert.match(text, /efx_FragColor=texture\(tex,uv\)/);
  assert.match(text, /#define SAMPLES 4/);
  assert.equal(expandIncludes('#include <a>', { a: '#include <b>', b: 'ok' }), 'ok');
  assert.throws(() => expandIncludes('#include <a>', { a: '#include <a>' }), /Recursive/);
  assert.throws(() => expandIncludes('#include <missing>', {}), /Unknown/);
  assert.equal(rewriteIdentifiers('/* vec3 */ vec3 x;', { vec3: 'float3' }), '/* vec3 */ float3 x;');
  const assembled=prepareGLSL('#version 300 es\n#define varying in\n#define texture2D texture\n#define gl_FragColor pc_color\nout vec4 pc_color; varying vec2 uv; void main(){gl_FragColor=vec4(uv,0.,1.);}');
  assert.match(assembled,/#define varying in/);
  assert.doesNotMatch(assembled,/#define in in|out vec4 efx_FragColor/);
});

test('real compiler preserves loops, functions, matrix and sampler resources', { skip: !process.env.GLSLANG_VALIDATOR || !process.env.SPIRV_CROSS }, async () => {
  const result = await compileGLSL(`uniform sampler2D field; uniform float dt; varying vec2 uv;
    float sum(vec2 p){float v=0.;for(int i=0;i<4;i++){v+=texture2D(field,p+vec2(float(i)*.01)).x;}return v;}
    void main(){mat2 m=mat2(1.,2.,3.,4.); gl_FragColor=vec4(m*uv,sum(uv)*dt,1.);}`);
  assert.equal(result.reflection.textures[0].name, 'field');
  assert.match(result.hlsl, /Texture2D/);
  assert.match(result.hlsl, /sum/);
  await assert.rejects(compileGLSL('void main(){gl_FragColor=missing;}'), /failed/);
});
