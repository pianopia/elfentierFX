# Three.js → UE5 コンバーター

three.jsの実行中のSceneを、UE5で使用できる**ネイティブアセット**へ変換します。
外部three.jsプロジェクト向けの独立したアダプターです。elfentierFXのデスクトップ
描画は既存のRust/wgpuを使い続けます。

| 表現 | 出力 / UE側 | 対応範囲 |
|---|---|---|
| モデル | `scene.glb` → StaticMesh / SkeletalMesh | BufferGeometry、UV、法線、頂点カラー、複数マテリアル、スキン、指定したAnimationClip |
| PBR | glTF → UEマテリアル | Standard/Physical/Basic。テクスチャは画像ロード済みのブラウザー経路。Physical拡張のUE対応は要確認 |
| GLSL | HLSL → `MaterialExpressionCustom` | スカラー/ベクトル式、uniformパラメーター、UV、Time。RGBAをUnlitのEmissive/Opacityへ接続 |
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

自動抽出は、標準的な位置変換のvertex shaderと、`main()`内の
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
sampler/matrix、ヘルパー関数、ループ、プリプロセッサ、TSL/WGSL、任意の
`onBeforeCompile`変更は自動変換しません。元のGLSL全体をHLSLとして実行する機能はありません。

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
GPU RenderTarget/GPGPUの流体は、元アプリでreadbackまたはCPUの表面生成を行ってから
上の形式へ渡してください。カメラ依存の水面画像から流体メッシュは復元できません。
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
- 本体デスクトップへの専用取り込みUI、Niagara/Waterの自動グラフ生成、リアルタイムGPUソルバーの移植は含みません。

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

2026-10-03のローカル検証: JavaScript 14件、Python事前検証7件、Rustコア72件、
VDB CLI/往復3件が成功。Blender 4.4でAlembicの最初/最後の頂点位置を往復確認。
UE5.8.2でモデル3個、4フレームGeometryCache、static SVT 2個、Custom/water
マテリアルの割り当てとVector4パラメーターを検証し、`ELFENTIER_UE_SMOKE_PASS`を確認しました。
テクスチャ付きブラウザー出力、SkeletalMesh取り込み、GPU描画の視覚比較は未検証です。

参照:
[three.js GLTFExporter](https://threejs.org/docs/pages/GLTFExporter.html)、
[UE Custom material nodes](https://dev.epicgames.com/documentation/en-us/unreal-engine/custom-material-expressions-in-unreal-engine)、
[UE Sparse Volume Textures](https://dev.epicgames.com/documentation/en-us/unreal-engine/sparse-volume-textures-in-unreal-engine)。
