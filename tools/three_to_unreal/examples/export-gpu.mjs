import { exportNativeGraph } from '../src/native-graph.js';
// Deterministic two-pass fixture. Density must read OLD velocity at every epoch.
const graph={format:'elfentier_gpu_graph_v1',name:'Fluid',width:2,height:1,semantics:'simultaneous_previous_epoch',variables:[
  {name:'velocity',shader:'uniform float dt;void main(){vec2 uv=gl_FragCoord.xy/resolution;gl_FragColor=texture2D(velocity,uv)+vec4(dt);}',dependencies:['velocity'],initial:[1,2,3,4,5,6,7,8],uniforms:{dt:{value:.1,binding:'delta'}},wrapS:'clamp',wrapT:'clamp',filter:'nearest'},
  {name:'density',shader:'void main(){vec2 uv=gl_FragCoord.xy/resolution;gl_FragColor=texture2D(velocity,uv)*2.0+texture2D(density,uv);}',dependencies:['velocity','density'],initial:[1,2,3,4,5,6,7,8],uniforms:{},wrapS:'clamp',wrapT:'clamp',filter:'nearest'},
]};
const result=await exportNativeGraph(graph,process.argv[2]);
console.log(`GPU_NATIVE_EXPORT ${result.root}; component ${result.component}`);
