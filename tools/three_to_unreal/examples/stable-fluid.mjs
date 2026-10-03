import { exportNativeGraph } from '../src/native-graph.js';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** Five-pass 2D Eulerian solver: advection, divergence, Jacobi pressure,
 * projection, dye transport. A compute() epoch samples only the old epoch,
 * matching GPUComputationRenderer's feedback semantics. */
export function fluidGraph(width=8,height=4) {
  const initial=(fn)=>Array.from({length:width*height},(_,i)=>fn(i%width,Math.floor(i/width))).flat();
  const velocity=initial((x,y)=>[Math.fround(Math.sin(x*.7)*1.75),Math.fround(Math.cos(y*.8)*.65),0,1]);
  const dye=initial((x,y)=>[Math.fround(Math.exp(-((x-3)**2+(y-1)**2)*.3)),.2,.05,1]);
  const zero=initial(()=>[0,0,0,1]);
  const prefix='void main(){vec2 uv=gl_FragCoord.xy/resolution;vec2 px=1.0/resolution;';
  const defs=[
    ['advected',['velocity'],'uniform float dt;'+prefix+'vec2 v=texture2D(velocity,uv).xy;gl_FragColor=vec4(texture2D(velocity,uv-v*dt*px).xy,0.,1.);}',velocity,true],
    ['divergence',['velocity'],prefix+'float d=(texture2D(velocity,uv+vec2(px.x,0)).x-texture2D(velocity,uv-vec2(px.x,0)).x+texture2D(velocity,uv+vec2(0,px.y)).y-texture2D(velocity,uv-vec2(0,px.y)).y)*.5;gl_FragColor=vec4(d,0.,0.,1.);}',zero,false],
    ['pressure',['pressure','divergence'],prefix+'float p=(texture2D(pressure,uv+vec2(px.x,0)).x+texture2D(pressure,uv-vec2(px.x,0)).x+texture2D(pressure,uv+vec2(0,px.y)).x+texture2D(pressure,uv-vec2(0,px.y)).x-texture2D(divergence,uv).x)*.25;gl_FragColor=vec4(p,0.,0.,1.);}',zero,false],
    ['velocity',['advected','pressure'],prefix+'vec2 g=vec2(texture2D(pressure,uv+vec2(px.x,0)).x-texture2D(pressure,uv-vec2(px.x,0)).x,texture2D(pressure,uv+vec2(0,px.y)).x-texture2D(pressure,uv-vec2(0,px.y)).x)*.5;gl_FragColor=vec4(texture2D(advected,uv).xy-g,0.,1.);}',velocity,false],
    ['dye',['velocity','dye'],'uniform float dt;'+prefix+'vec2 v=texture2D(velocity,uv).xy;gl_FragColor=vec4(texture2D(dye,uv-v*dt*px).rgb*.99,1.);}',dye,true],
  ];
  return {format:'elfentier_gpu_graph_v1',name:'StableFluid',width,height,semantics:'simultaneous_previous_epoch',variables:defs.map(([name,dependencies,shader,initial,dt])=>({name,dependencies,shader,initial:[...initial],uniforms:dt?{dt:{value:.35,binding:'delta'}}:{},wrapS:'clamp',wrapT:'clamp',filter:'nearest'}))};
}

/** Independent CPU reference uses integer nearest/clamped cell accesses.
 * New arrays are committed together; pass ordering must not change results. */
export function referenceFluid(graph,delta,iterations) {
  const {width:w,height:h}=graph;let state=Object.fromEntries(graph.variables.map(v=>[v.name,[...v.initial]]));
  const index=(x,y)=>4*(Math.max(0,Math.min(h-1,Math.floor(y)))*w+Math.max(0,Math.min(w-1,Math.floor(x))));
  for(let step=0;step<iterations;step++){
    const next=Object.fromEntries(Object.keys(state).map(key=>[key,new Array(w*h*4)]));
    const at=(name,x,y,c=0)=>state[name][index(x,y)+c];
    for(let y=0;y<h;y++)for(let x=0;x<w;x++){
      const i=index(x,y),vx=at('velocity',x,y),vy=at('velocity',x,y,1);
      const sx=x+.5-vx*delta,sy=y+.5-vy*delta;
      const div=.5*(at('velocity',x+1,y)-at('velocity',x-1,y)+at('velocity',x,y+1,1)-at('velocity',x,y-1,1));
      const p=.25*(at('pressure',x+1,y)+at('pressure',x-1,y)+at('pressure',x,y+1)+at('pressure',x,y-1)-at('divergence',x,y));
      const gx=.5*(at('pressure',x+1,y)-at('pressure',x-1,y)),gy=.5*(at('pressure',x,y+1)-at('pressure',x,y-1));
      const rows={advected:[at('velocity',sx,sy),at('velocity',sx,sy,1),0,1],divergence:[div,0,0,1],pressure:[p,0,0,1],velocity:[at('advected',x,y)-gx,at('advected',x,y,1)-gy,0,1],dye:[at('dye',sx,sy)*.99,at('dye',sx,sy,1)*.99,at('dye',sx,sy,2)*.99,1]};
      for(const [name,values]of Object.entries(rows))values.forEach((value,c)=>{next[name][i+c]=Math.fround(value);});
    }
    state=next;
  }
  return state;
}
if(process.argv[1]===fileURLToPath(import.meta.url)){
  const graph=fluidGraph();const port=await exportNativeGraph(graph,process.argv[2]);
  await writeFile(join(port.root,'expected-reference.json'),JSON.stringify({width:graph.width,height:graph.height,delta:.35,iterations:6,state:referenceFluid(graph,.35,6)},null,2)+'\n',{flag:'wx'});
  console.log('STABLE_FLUID_NATIVE_EXPORT '+port.root);
}
