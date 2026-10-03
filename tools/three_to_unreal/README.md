# Three.js → UE5 コンバーター

three.jsの実行中のSceneを、UE5で使用できる**ネイティブアセット**へ変換します。
外部three.jsプロジェクト向けの独立したアダプターです。elfentierFXのデスクトップ
描画は既存のRust/wgpuを使い続けます。

| 表現 | 出力 / UE側 | 対応範囲 |
|---|---|---|
| モデル | `scene.glb` → StaticMesh / SkeletalMesh | BufferGeometry、UV、法線、頂点カラー、複数マテリアル、スキン、指定したAnimationClip |
| PBR | glTF → UEマテリアル | Standard/Physical/Basic。テクスチャは画像ロード済みのブラウザー経路。Physical拡張のUE対応は要確認 |
| GLSL | SPIR-V → HLSL / `MaterialExpressionCustom` | 全体の言語変換と、明示的なリソース・座標バインドによるUE接続。簡易式変換も対応 |
| GPUソルバー | 生成Runtimeプラグイン → GlobalShader / RDG | 捕捉したGPUComputationRendererグラフ全体、RGBA32f、同時ping-pong更新 |
| 水面 | UEのLitマテリアル + WPO | 指定した正弦波をワールド座標で再生。流体ソルバーの移植ではありません |
| 流体表面 | JSONスナップショット → Blender → `.abc` → GeometryCache | 三角形の固定トポロジー、全頂点のフレームごとの位置。fpsを保持 |
| 煙・密度 | `.evol` → Rust → `.vdb`列 → SparseVolumeTexture | CPUのスカラー密度グリッド、複数フレーム。UE用cm/Z-upへ座標変換 |

## セットアップ

Node.js 20以上、Rust stable。Alembic変換にはAlembic対応のBlenderが必要です。

```sh
# リポジトリのルート
cargo build -p vdb_convert --release
cd tools/three_to_unreal
npm ci
node src/cli.js --help
```

WindowsではRust用のMSVC Build Toolsが必要です。VDB出力は既存のpure-Rust
OpenVDB writerを使用するため、OpenVDB Pythonモジュールのインストールは不要です。

## 1. Nodeでエクスポート

入力は信頼できるローカルES moduleです。`createScene()`またはdefault exportで
`{ scene, animations?, volumes?, surfaceCaches?, name? }`を返します。
CLIはこのモジュールを通常のJavaScriptとして実行します。

```sh
# tools/three_to_unrealから（実行ファイルのパスは環境に合わせる）
node src/cli.js examples/scene.mjs --out ../../artifacts/my-three-export \
  --blender /path/to/blender \
  --vdb-converter ../../target/release/vdb_convert
```

Windowsの実行ファイルは`blender.exe`、`vdb_convert.exe`です。出力先は空の
ディレクトリを指定します。既存ファイルは上書きしません。
Nodeには画像/canvasのDOMがないため、**テクスチャのあるシーンは次のブラウザー経路**
を使ってください。単にURLが設定されているだけでは画像を書き出せません。

オプションなしでもGLB、元のFXデータ、マニフェスト、診断は生成できます。
`conversion-report.json`の`pending`が空になるまで、全FXのネイティブ変換は完了していません。

## 2. 実行中のthree.jsアプリからZIP保存

ソースアプリでこのパッケージをローカル依存として追加するか、Vite等でsrcを
importします。依存three.jsはr180に固定して検証しています。他バージョンとの
互換性はソースアプリで確認してください。

```js
import { downloadThreeBundle } from '@elfentierfx/three-to-unreal/download';

exportButton.onclick = async () => {
  // テクスチャ画像がロード済みのscene。CORSによるcanvas汚染は解消しておく。
  await downloadThreeBundle(scene, {
    name: 'MyEffect', animations: clips,
    volumes: densityVolumes, surfaceCaches: liquidSurfaces,
  });
};
```

ディスク書き込みを行わないAPIもあります。

```js
import { exportThreeBundle } from '@elfentierfx/three-to-unreal';
const { files, manifest } = await exportThreeBundle(scene);
// files: Map<相対ファイル名, Uint8Array>
```

ZIPを展開した後、ネイティブFXだけ追加で変換できます。

```sh
node src/cli.js bake /path/to/unpacked-bundle \
  --blender /path/to/blender --vdb-converter /path/to/vdb_convert
```

## シェーダーの変換

簡易式変換の自動抽出は、標準的な位置変換のvertex shaderと、`main()`内の
`gl_FragColor = 式;`だけで構成されたfragment shaderに限定します。
一般のShaderMaterialには、意図したRGBA式を明示します。

```js
material.userData.elfentierUE = {
  fragmentExpression: 'vec4(mix(tint, vec3(1.0), 0.5 + 0.5 * sin(t)), 1.0)',
  bindings: { vUv: 'uv', t: 'time' },
};
material.uniforms.tint = { value: new THREE.Color(0.1, 0.3, 0.8) };
```

`UV`（vec2）、`Time`（float）は組み込みです。uniformの数値、Color、Vector2/3/4を
UEパラメーターへ変換します。名前変更や関数名の置換だけでなく、式の構文・
ベクトル次元を検査します。`mix → lerp`、`fract → frac`、GLSLの負値の`mod`
（floor基準）、二引数`atan → atan2`の意味を保持します。

対応: `vec2/3/4`、演算`+ - * /`、swizzle、`sin cos abs floor ceil fract sqrt exp log
normalize length min max pow dot cross step mix clamp smoothstep mod atan`。
結果はvec4である必要があります。

明示アダプターは指定したfragment式だけを出力します。元のvertex変形・照明・
raymarchingの再現を保証しません。元のvertex/fragment GLSLは診断用JSONに保存します。
この簡易経路はsampler/matrix、ヘルパー関数、ループ、プリプロセッサ、TSL/WGSL、任意の
`onBeforeCompile`変更を変換しません。GLSL全体の変換には下記のコンパイラー経路を使います。

水面のアダプター:

```js
waterMaterial.userData.elfentierUE = { water: {
  color: [0.015, 0.12, 0.16], roughness: 0.08,
  waves: [{ direction: [1, 0.3], amplitude: 0.08, wavelength: 2.5, speed: 1.3 }],
} };
```

振幅/波長はm、速度はrad/s、directionはthree.jsのXZ方向です。
十分に細分化した水面メッシュを使用してください。泡、屈折、波面の法線更新、
水中散乱、衝突、圧力計算は含みません。

## 流体・ボリュームの入力

```js
const surfaceCaches = [{
  name: 'Waterfall', fps: 30,
  indices: Uint32Array.of(0, 1, 2), // 全フレームで同じ三角形トポロジー
  frames: [positionsAtT0, positionsAtT1], // xyzの平坦配列、m / Y-up
}];
const volumes = [{
  name: 'Smoke', resolution: [nx, ny, nz],
  boundsMin: [-1, 0, -1], boundsMax: [1, 2, 1],
  frames: [densityAtT0, densityAtT1], // 非負f32、x + nx*(y + ny*z)
}];
```

`volumeFromTexture(texture, { boundsMin, boundsMax })`はCPUデータを持つ
RedFormatのFloat/UnsignedByte `Data3DTexture`を抽出します。Byteは0..1へ正規化。
GPU RenderTargetからキャッシュをベイクする場合は、元アプリでreadbackまたはCPUの表面生成を行ってから
上の形式へ渡してください。GPU計算をUEで再実行する場合は下記のグラフ経路を使います。
カメラ依存の水面画像から流体メッシュは復元できません。
固定トポロジー以外のメッシュ列とFLIP粒子からの表面再構成は未対応です。

`vdb_convert`単体でも使用可能です。

```sh
cargo run -p vdb_convert -- to-vdb out/density.vdb input.evol --sequence --ue-space
# out/density_0000.vdb, out/density_0001.vdb, ...
```

glTFはm/Y-upのままUE Interchangeに渡します。VDBは軸(x,y,z)→(x,z,y)、位置と
境界を100倍してcmへ変換します。密度値は変更しません（writerの活性化閾値1e-7）。
AlembicはBlenderがY-up/mで書き出し、UEインポーターに軸/単位変換を明示します。
複数回のスケール/軸変換を重ねないでください。

## UE Editorへの取り込み

UE5.3以上を対象とします。Python Editor Script Plugin、Editor Scripting Utilities、
GLBに対応するInterchange、Alembic Importerを有効にしてください。
スクリプトは[UEプラグイン](../../integrations/unreal/ElfentierFX/README.md)の
`Content/Python/import_three_bundle.py`にあります。C++モジュールをビルドせず、
このPythonファイル単体をEditorでロードすることもできます。

UEのPythonコンソール:

```python
import sys
sys.path.insert(0, r'C:/path/to/elfentierFX/integrations/unreal/ElfentierFX/Content/Python')
import import_three_bundle
result = import_three_bundle.import_bundle(r'C:/exports/MyEffect', '/Game/MyEffect')
# Headless Editorで保存済み材質を読み直して描画する場合:
import_three_bundle.refresh_native_materials(result)
```

出力先は空のContentフォルダーを指定してください。先に全ペイロードの有無、
サイズ、座標情報、未対応診断を検証します。その後GLB/GeometryCache/SVTを
インポートし、Custom HLSL/水面のマテリアルを生成してStaticMeshのスロットへ割り当てます。
ネイティブマテリアルが作成できてもスロット割り当てに失敗したらエラーになります。
インポートはトランザクションではなく、途中失敗時に既に作成されたアセットは残ります。

VDBは**各フレームを個別のstatic SVTとしてインポート**します。自動Animated SVT
生成・再生はUEバージョンごとの設定に依存するため未対応です。アニメーションは
Content BrowserでVDB列の先頭を**Import Sequence**付きで再インポートしてください。
HeterogeneousVolumeを配置し、Volume-domainマテリアルのSVTへ割り当てます。
Channel mappingや消散/散乱係数はUEで調整してください。

## 診断と制約

- デフォルトstrict: 未対応のシェーダー、フック、points/lines/sprites、独自GPU属性などは変換を失敗させます。
- `--allow-partial`: 調査用のGLBと診断を出力。未対応材質はマゼンタになり、CLI終了コードは2。UEインポーターはこの不完全バンドルを拒否します。
- 環境光、HDR、フォグ、ポストエフェクト、画面空間屈折、トーンマッピング、アクターの配置/シーン再構築はUEで設定します。ピクセル一致は保証しません。
- スキン/AnimationClipはGLBで保持しますが、UE SkeletalMeshの設定はソースに応じて調整が必要です。
- インスタンスは最大10000件まで展開。大量の草/都市には元アプリ側の結合・ベイクを推奨。
- 密度データは512 MiBまで、各軸512以下。表面キャッシュも入力上限を検査します。
- 本体デスクトップへの専用取り込みUI、Niagara/Waterの自動グラフ生成は含みません。GPUソルバーのネイティブ生成は下記の明示的なグラフ経路で対応します。

## GLSL全体とネイティブGPUグラフ

式だけでなく、関数・ループ・行列・テクスチャを含むGLSLを公式の
[glslang](https://github.com/KhronosGroup/glslang)でSPIR-Vへコンパイルし、
[SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross)でHLSLへ変換します。
両実行ファイルをPATHへ入れるか、`GLSLANG_VALIDATOR`と`SPIRV_CROSS`環境変数を設定します。
WindowsではKhronosのソースをCMakeでビルドできます。LinuxのCIはglslang-tools/spirv-crossを使用します。

```powershell
node src/cli.js shader water.frag --stage frag --out exports/water
node src/cli.js shader solver.comp --stage comp --vulkan --out exports/solver
```

`vert`/`frag`/`comp`をサポート。GLSL、SPIR-V、HLSL、リフレクションとコンパイル診断を出力します。
WebGL互換モードはattribute/varying/gl_FragColor/texture2Dを現行構文に置換し、ShaderChunkを展開します。
Rendererが注入する属性・uniform・defineは、元Rendererの完全なシェーダーを渡すか、
`compileGLSL(source, {prefix, defines, chunks})`で指定してください。`chunks`はソースと同じthree.js版の辞書を使います。
GLSLの構文・型はコンパイラーが検査し、失敗を成功扱いにはしません。
出力の`languageCompiled`は言語変換の成功です。UEの描画パイプラインへの接続を表すものではありません。

ブラウザーのGPUComputationRendererをグラフ化:

```js
import { captureGPUComputation } from '@elfentierfx/three-to-unreal/gpu-graph';
const graph = captureGPUComputation(gpuCompute, {
  name: 'MyFluid',
  bindings: { 'velocity.dt': 'delta', 'velocity.time': 'time' }
});
// graphをJSONとして保存。CPU初期値からの再実行を捕捉し、進行中GPU状態は読まない。
```

```powershell
node src/cli.js gpu captured-graph.json --out exports/ElfentierGpuMyFluid
```

生成プラグインをUEプロジェクトの`Plugins/ElfentierGpuMyFluid`へコピーし、Editorをビルドします。
UE5.8.2で検証したRuntimeモジュールです。各変数をGlobalShaderへ変換し、RDGで実行します。
`ElfentierMyFluidComponent`をActorへ追加し、`Reset()`、`Step(delta, iterations)`、
`GetOutput('velocity')`でRGBA32fのUTextureRenderTarget2Dを取得します。マテリアルから通常のTextureとして参照できます。
`SetUniform('velocity.force', Vector4)`で定数を変更します。time/delta/frameの明示的なバインドは自動更新します。

全パスが同じ前フレームを読み、全パス終了後に新しい状態へ交換します。
GPUComputationRendererの`compute()`を1回呼ぶことと`Step(delta,1)`が対応します。
**圧力パスのみを反復するJSループ**などは、その実行スケジュールを別途アダプターで定義する必要があります。
同じグラフを`iterations`回繰り返す場合は、グラフ全体を毎回更新します。

対応範囲は2D RGBA Float32、初期CPUデータ、最大32変数/4096平方/初期状態512MiB、
nearest/linearとclamp/repeat/mirror、float/vec2/vec3/vec4のuniformです。
HalfFloat、外部テクスチャ、独自SSBO/image、MRT、fragment derivatives/discardを使うソルバーは
明示的なアダプターが必要です。JSのコールバック、GUI、入力、音、非公開のRenderer状態を
任意のソースから推測する万能な自動移植ではありません。未知のリソースは診断して停止します。

`native-port-report.json`は生成段階ではnativeBuildVerified/gpuExecutionVerifiedをfalseにします。
生成できたこと、UEビルドが通ったこと、GPUで正しく実行したことを区別してください。

## three-ocean-beachでの実ソース検証

ローカルのthree-ocean-beachのファイルを読み、元プロジェクトは編集しません。

```powershell
node examples/compile-ocean.mjs C:/path/three-ocean-beach C:/exports/ocean-shaders
node examples/export-ocean.mjs C:/path/three-ocean-beach C:/exports/ocean-native
node examples/stable-fluid.mjs C:/exports/ElfentierGpuStableFluid
```

空・砂浜・水面・草の計8シェーダーを元three.js版でコンパイルします。
空/砂浜/水面の重点検証バンドルは実際の砂テクスチャとGLBを出力し、
元のGLSL関数を含むCustom HLSL、頂点変位用WPO、UE SceneColor/SceneDepthの屈折を生成します。
UEがトーンマッピングするため、WebGLの末尾トーンマッピング/色空間変換は取り除きます。
草9000本、石、岬、GUI/音、カメラ追従はこの重点検証バンドルに含みません。
海は数式による水面表現であり、GPU流体ソルバーではありません。

`examples/stable-fluid.mjs`は移流、発散、Jacobi圧力、射影、染料輸送の5パスを生成します。
`test/ue_gpu.py`はUE上の全RGBA値を独立CPU参照計算と照合します。
`test/ue_ocean.py`は実RHIで海のHLSLコンパイルと資産割り当てを確認します。
`test/ue_ocean_render.py`は読み直した材質を再コンパイルし、空/砂浜/水面を
SceneCapture2DでPNGへ描画します。UE5.8のheadless Editorで読み直した直後の
描画には`refresh_native_materials(result)`が必要でした。これはEditor用の処理で、
配布するプロジェクトでは通常のUEビルド・Cookを行います。
これらは生成したプラグインをビルドした空の検証プロジェクトで実行してください。
描画のピクセル一致やあらゆるthree.jsアプリケーションの再現を保証しません。

## 検証

```sh
cd tools/three_to_unreal && npm test
# ルートから
cargo test -p vdb_convert
cargo test -p elfentier_core
python -m unittest discover -s integrations/unreal/tests -v
```

`test/ue_smoke.py`は空のUE Editor検証プロジェクトでGLB、GeometryCache、
マテリアルの実インポートを検査するためのスクリプトです。
`ELFENTIER_BUNDLE`と`ELFENTIER_RESULT`を設定して
`UnrealEditor-Cmd <project.uproject> -unattended -nullrhi -ExecutePythonScript=<absolute-script-path>`
で実行します。NullRHIの検証はアセット生成・割り当て確認であり、視覚的一致や
GPU上でのシェーダー品質を検証するものではありません。

基本アセット経路の2026-10-03検証: JavaScript 14件、Python事前検証7件、Rustコア72件、
VDB CLI/往復3件が成功。Blender 4.4でAlembicの最初/最後の頂点位置を往復確認。
UE5.8.2でモデル3個、4フレームGeometryCache、static SVT 2個、Custom/water
マテリアルの割り当てとVector4パラメーターを検証し、`ELFENTIER_UE_SMOKE_PASS`を確認しました。
拡張後はJavaScript 22件、Python 8件、実UEの5パス流体GPU結果640値も検証しました。
詳細は[検証記録](../../docs/three_to_unreal_validation.md)を参照してください。
テクスチャ付きブラウザー出力、SkeletalMesh取り込み、WebGLとのピクセル比較は未検証です。

参照:
[three.js GLTFExporter](https://threejs.org/docs/pages/GLTFExporter.html)、
[UE Custom material nodes](https://dev.epicgames.com/documentation/en-us/unreal-engine/custom-material-expressions-in-unreal-engine)、
[UE Sparse Volume Textures](https://dev.epicgames.com/documentation/en-us/unreal-engine/sparse-volume-textures-in-unreal-engine)。
