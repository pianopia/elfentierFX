// Explicit render-pipeline adapter for three-ocean-beach. Source is read-only.
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { writeFile } from 'node:fs/promises';
import { Scene, Mesh, PlaneGeometry, SphereGeometry, ShaderMaterial, DoubleSide, BackSide } from 'three';
import { compileMaterial } from '../src/material-compiler.js';
import { exportToDirectory } from '../src/node.js';

const root = resolve(process.argv[2]), out = resolve(process.argv[3]);
const source = await import(pathToFileURL(join(root, 'src/shaders.js')).href);
const { ShaderChunk } = await import(pathToFileURL(join(root, 'node_modules/three/build/three.module.js')).href);
const { createSandTextures } = await import(pathToFileURL(join(root, 'src/sand-textures.js')).href);
const sand = createSandTextures();
const uniforms = { uWave: .65, uCloud: .3, uGolden: 1, uSun: [-.495134, .139173, -.857598], uCameraFar: 5500 };
// Explicit nodes also tell UE which per-pixel data the Custom node requires.
const bindings = { uTime: { input: { name: 'Time', kind: 'time' }, code: 'Time' },
  cameraPosition: { input: { name: 'Camera', kind: 'camera_position' }, code: 'Camera.xzy / 100.0' },
  vWorld: { input: { name: 'World', kind: 'world_position' }, code: 'World.xzy / 100.0' } };
const textures = { uSandAlbedo: 'textures/SandAlbedo.tga', uSandNormal: 'textures/SandNormal.tga' };
const scene = new Scene(), recipes = [];
for (const name of ['sky', 'terrain', 'water']) {
  let fragment = source[name + 'Fragment'].replace(/#include <(?:tonemapping_fragment|colorspace_fragment)>/g, '');
  const options = { name: name, prefix: 'uniform vec3 cameraPosition;', chunks: ShaderChunk, bindings: { ...bindings }, uniforms, textures: { ...textures } };
  if (name === 'sky') options.bindings.vWorld = { input: { name: 'ViewRay', kind: 'camera_vector' }, code: '-ViewRay.xzy' };
  if (name === 'water') {
    // Three uses normalized OpenGL depth. UE samples linear centimeters instead.
    const changes = [
      [/vec2 uv=gl_FragCoord.xy\/uResolution;/, 'vec2 uv=efxViewportUV;'],
      [/float sceneZ=-perspectiveDepthToViewZ\(texture2D\(uSceneDepth,bentUV\).x,uCameraNear,uCameraFar\);/, 'float sceneZ=efxDepth(bentUV);'],
      [/float waterZ=-perspectiveDepthToViewZ\(gl_FragCoord.z,uCameraNear,uCameraFar\);/, 'float waterZ=efxPixelDepth;'],
      [/texture2D\(uSceneColor,bentUV\).rgb/, 'efxColor(bentUV)'],
      [/if\(waterDepth<=0.0\) discard;/, 'if(waterDepth<=0.0){gl_FragColor=vec4(0);return;}'],
    ];
    for (const [pattern, replacement] of changes) {
      if (!pattern.test(fragment)) throw new Error('Ocean source changed; update the explicit UE refraction adapter');
      fragment = fragment.replace(pattern, replacement);
    }
    options.prefix += '\nvarying vec2 efxViewportUV; varying float efxPixelDepth;\nfloat efxDepth(vec2 uv){return texture2D(uSceneDepth,uv).x;}\nvec3 efxColor(vec2 uv){return texture2D(uSceneColor,uv).rgb;}';
    // Resource declarations must precede helper functions.
    options.prefix = 'uniform sampler2D uSceneDepth; uniform sampler2D uSceneColor;\n' + options.prefix;
    fragment = fragment.replace(/uniform sampler2D uScene(?:Color|Depth);/g, '');
    options.bindings.efxViewportUV = { input: { name: 'ScreenUV', kind: 'screen_uv' }, code: 'ScreenUV.xy' };
    options.bindings.efxPixelDepth = { input: { name: 'PixelDepth', kind: 'pixel_depth' }, code: 'PixelDepth/100.0' };
    options.textures.uSceneDepth = options.textures.uSceneColor = null;
    options.extraFields = 'FMaterialPixelParameters NativeParameters;';
    options.setup = 'fx.NativeParameters = Parameters;';
    options.overrides = {
      efxDepth: 'return CalcSceneDepth(ViewportUVToBufferUV(uv)) / 100.0;',
      efxColor: 'return DecodeSceneColorForMaterialNode(ViewportUVToBufferUV(uv));',
    };
  }
  const { recipe } = await compileMaterial(fragment, options);
  if (name === 'water') {
    // Nodes tell UE's compiler to provide scene color/depth resources.
    recipe.inputs.push({ name: 'NativeSceneColor', kind: 'scene_color' }, { name: 'NativeSceneDepth', kind: 'scene_depth' });
    recipe.transparent = true;
  }
  recipe.twoSided = true;
  if (name !== 'sky') {
    const height = name === 'water' ? 'swell(vWorld.xz)+.015' : 'land(vWorld.xz)';
    const wpoSource = source.common + `void main(){gl_FragColor=vec4(0.0, ${height}-vWorld.y, 0.0, 0.0);}`;
    const wpo = await compileMaterial(wpoSource, { name: name + '_WPO', bindings, uniforms, textures, chunks: ShaderChunk });
    recipe.wpo = { ...wpo.recipe, output: 'float3', hlsl: wpo.recipe.hlsl.replace(/return fx\.efx_FragColor;/, 'return fx.efx_FragColor.xzy*100.0;') };
  }
  recipes.push(recipe);
  let geometry;
  if (name === 'sky') geometry = new SphereGeometry(2400, 40, 24);
  else {
    const nx = name === 'water' ? 380 : 320, nz = name === 'water' ? 330 : 240;
    geometry = new PlaneGeometry(1, 1, nx, nz); geometry.rotateX(-Math.PI / 2);
    const attr = geometry.attributes.position;
    for (let j=0;j<=nz;j++) for (let i=0;i<=nx;i++) {
      const t=i/nx, s=j/nz;
      const x=Math.sign(t*2-1)*Math.pow(Math.abs(t*2-1),name==='water'?1.7:1.6)*(name==='water'?1500:500);
      const z=name==='water'?30-Math.pow(s,2.6)*2100:15+Math.sign(s*2-1)*Math.pow(Math.abs(s*2-1),2)*410;
      attr.setXYZ(j*(nx+1)+i,x,0,z);
    }
    geometry.computeVertexNormals(); geometry.computeBoundingSphere();
  }
  const material = new ShaderMaterial({ side: name==='sky'?BackSide:DoubleSide });
  material.name = name; material.userData.elfentierUE = { compiled: recipe };
  const mesh = new Mesh(geometry, material); mesh.name=name; scene.add(mesh);
}
await exportToDirectory(scene, out, { name: 'OceanBeach' });
// TGA supports lossless, CPU-backed RGBA textures without browser/canvas tools.
function tga(texture) {
  const {width,height,data} = texture.image;
  const bytes = Buffer.alloc(18+width*height*4); bytes[2]=2; bytes.writeUInt16LE(width,12); bytes.writeUInt16LE(height,14); bytes[16]=32; bytes[17]=0x28;
  for(let i=0;i<width*height;i++){bytes[18+i*4]=data[i*4+2];bytes[19+i*4]=data[i*4+1];bytes[20+i*4]=data[i*4];bytes[21+i*4]=data[i*4+3];}
  return bytes;
}
const { mkdir } = await import('node:fs/promises'); await mkdir(join(out,'textures'));
await writeFile(join(out,'textures/SandAlbedo.tga'),tga(sand.albedo),{flag:'wx'});
await writeFile(join(out,'textures/SandNormal.tga'),tga(sand.normal),{flag:'wx'});
await writeFile(join(out,'ocean-adapter.json'),JSON.stringify({ source: root, scope: ['sky','terrain','water'],
  remaining: ['9000 grass instances, stones and cape are not included in this focused ocean material fixture','Native refraction uses UE scene color/depth; WebGL depth and tonemapping are replaced','Sky actor must follow camera','Enlarge mesh bounds for vertex displacement'] },null,2)+'\n',{flag:'wx'});
console.log('OCEAN_NATIVE_EXPORT_PASS '+out);
