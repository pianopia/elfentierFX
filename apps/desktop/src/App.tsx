import { useCallback, useEffect, useRef, useState } from "react";
import { agentApi } from "./agent/api";
import NodeCanvas from "./components/NodeCanvas";
import PromptBar from "./components/PromptBar";
import SplitPane from "./components/SplitPane";
import type {
  CookResult,
  ExportResult,
  Graph,
  NativePreviewImage,
  NativeViewportCamera,
  SmokeExportResult,
  ViewportMesh,
} from "./types/graph";
import Viewport3D from "./viewport/Viewport3D";
import "./App.css";

function formatCookStatus(stats: CookResult): string {
  if (stats.smoke_particle_count != null && stats.smoke_particle_count > 0) {
    const res = stats.smoke_resolution?.join("×") ?? "?";
    return `Cooked "${stats.graph_name}": smoke ${res} · max ρ ${stats.smoke_max_density?.toFixed(2)} · ${stats.smoke_steps} steps · ${stats.smoke_frame_count} frames · ${stats.smoke_particle_count} particles`;
  }
  return `Cooked "${stats.graph_name}": ${stats.instance_count} instances · ${stats.vertex_count} verts · ${stats.triangle_count} tris`;
}

function App() {
  const [coreVersion, setCoreVersion] = useState("…");
  const [status, setStatus] = useState("Starting…");
  const [preset, setPreset] = useState<Graph | null>(null);
  const [graph, setGraph] = useState<Graph | null>(null);
  const [cookResult, setCookResult] = useState<CookResult | null>(null);
  const [viewportMesh, setViewportMesh] = useState<ViewportMesh | null>(null);
  const [nativePreview, setNativePreview] = useState<NativePreviewImage | null>(null);
  const [nativeCamera, setNativeCamera] = useState<NativeViewportCamera | null>(null);
  const [viewportError, setViewportError] = useState<string | null>(null);
  const [exportResult, setExportResult] = useState<ExportResult | null>(null);
  const [smokeExportResult, setSmokeExportResult] = useState<SmokeExportResult | null>(null);
  const [promptSummary, setPromptSummary] = useState("");
  const [explainText, setExplainText] = useState("");
  const [busy, setBusy] = useState(false);
  const graphRef = useRef<Graph | null>(null);

  const isSmokeGraph = graph?.nodes.some((n) => n.kind === "smoke_root") ?? false;

  const refreshExplain = useCallback(async (g: Graph) => {
    try {
      const text = await agentApi.explainGraph(g);
      setExplainText(text);
    } catch {
      setExplainText("");
    }
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      try {
        const [version, shopPreset] = await Promise.all([
          agentApi.getCoreVersion(),
          agentApi.getShopStreetPreset(),
        ]);
        if (cancelled) return;
        setCoreVersion(version);
        setPreset(shopPreset);
        setGraph(shopPreset);
        graphRef.current = shopPreset;
        setStatus(`Core ${version} ready — shop street preset loaded`);
        refreshExplain(shopPreset);
      } catch (error) {
        if (cancelled) return;
        setStatus(`Core unavailable: ${String(error)}`);
      }
    }

    bootstrap();
    return () => {
      cancelled = true;
    };
  }, [refreshExplain]);

  const handleGraphChange = useCallback(
    (next: Graph) => {
      graphRef.current = next;
      setGraph(next);
      refreshExplain(next);
    },
    [refreshExplain],
  );

  const loadGraphPreset = useCallback(
    async (label: string, loader: () => Promise<Graph>) => {
      setBusy(true);
      setStatus(`Loading ${label}…`);
      try {
        const nextPreset = await loader();
        setPreset(nextPreset);
        setGraph(nextPreset);
        graphRef.current = nextPreset;
        setCookResult(null);
        setViewportMesh(null);
        setNativePreview(null);
        setNativeCamera(null);
        setViewportError(null);
        setExportResult(null);
        setSmokeExportResult(null);
        setPromptSummary("");
        setStatus(`${label} loaded`);
        refreshExplain(nextPreset);
      } catch (error) {
        setStatus(`Preset failed: ${String(error)}`);
      } finally {
        setBusy(false);
      }
    },
    [refreshExplain],
  );

  const handleLoadShopPreset = useCallback(
    () => loadGraphPreset("Shop street preset", agentApi.getShopStreetPreset),
    [loadGraphPreset],
  );

  const handleLoadSmokePreset = useCallback(
    () => loadGraphPreset("Smoke puff preset", agentApi.getSmokePuffPreset),
    [loadGraphPreset],
  );

  const handleCook = useCallback(async () => {
    const current = graphRef.current;
    if (!current) {
      setStatus("No graph to cook");
      return;
    }
    setBusy(true);
    setStatus("Cooking graph…");
    try {
      const result = await agentApi.cook(current);
      setCookResult(result.stats);
      setViewportMesh(result.mesh);
      setNativePreview(result.native_preview ?? null);
      setNativeCamera(result.native_camera ?? null);
      setViewportError(result.native_preview_error ?? null);
      if (result.native_preview_error) {
        console.error("Native wgpu viewport preview failed:", result.native_preview_error);
      }
      setStatus(formatCookStatus(result.stats));
    } catch (error) {
      setStatus(`Cook failed: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }, []);

  const handleExport = useCallback(async () => {
    const current = graphRef.current;
    if (!current) {
      setStatus("No graph to export");
      return;
    }
    setBusy(true);
    if (isSmokeGraph) {
      setStatus("Exporting smoke density atlas…");
      try {
        const path = "/tmp/elfentier_smoke_density.raw";
        const result = await agentApi.exportSmokeDensity(current, path);
        setSmokeExportResult(result);
        setStatus(
          `Exported smoke atlas (${result.format}) → ${result.path} · ${result.frame_count} frames · ${result.byte_len} bytes`,
        );
      } catch (error) {
        setStatus(`Export failed: ${String(error)}`);
      } finally {
        setBusy(false);
      }
      return;
    }

    setStatus("Exporting glTF…");
    try {
      const path = "/tmp/elfentier_city.glb";
      const result = await agentApi.exportGltf(current, path);
      setExportResult(result);
      setStatus(
        `Exported ${result.byte_len} bytes → ${result.path} (${result.triangle_count} tris)`,
      );
    } catch (error) {
      setStatus(`Export failed: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }, [isSmokeGraph]);

  const handlePrompt = useCallback(
    async (prompt: string) => {
      const current = graphRef.current;
      if (!current) return;
      setBusy(true);
      setStatus(`Applying prompt: ${prompt}`);
      try {
        const result = await agentApi.applyPrompt(current, prompt);
        setGraph(result.graph);
        graphRef.current = result.graph;
        setPreset(result.graph);
        setPromptSummary(result.summary);
        setCookResult(null);
        setViewportMesh(null);
        setNativePreview(null);
        setNativeCamera(null);
        setViewportError(null);
        setExportResult(null);
        setSmokeExportResult(null);
        setStatus(result.summary || "Prompt applied");
        refreshExplain(result.graph);
      } catch (error) {
        setStatus(`Prompt failed: ${String(error)}`);
      } finally {
        setBusy(false);
      }
    },
    [refreshExplain],
  );

  return (
    <div className="app">
      <header className="app-header">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">◆</span>
          <h1>elfentierFX</h1>
          <span className="brand-tag">Alpha 2</span>
        </div>
        <p className="brand-subtitle">
          Building → city · smoke/gas fluids · 3D viewport · prompt-driven graph
        </p>
      </header>

      <PromptBar
        onSubmit={handlePrompt}
        summary={promptSummary}
        explainText={explainText}
        disabled={busy || !graph}
      />

      <main className="app-main">
        <SplitPane
          left={
            <NodeCanvas
              preset={preset}
              graphName={graph?.name ?? "Graph"}
              onGraphChange={handleGraphChange}
            />
          }
          right={
            <Viewport3D
              mesh={viewportMesh}
              nativePreview={nativePreview}
              nativeCamera={nativeCamera}
              initialError={viewportError}
            />
          }
        />
      </main>

      <footer className="app-footer">
        <span className="status-line">{status}</span>
        <div className="footer-actions">
          <span className="core-version">core {coreVersion}</span>
          <button
            type="button"
            className="action-button secondary"
            onClick={handleLoadShopPreset}
            disabled={busy}
          >
            Shop → Street
          </button>
          <button
            type="button"
            className="action-button secondary"
            onClick={handleLoadSmokePreset}
            disabled={busy}
          >
            煙 · Smoke puff
          </button>
          <button type="button" className="action-button" onClick={handleCook} disabled={busy}>
            Cook
          </button>
          <button type="button" className="action-button" onClick={handleExport} disabled={busy}>
            {isSmokeGraph ? "Export smoke" : "Export glTF"}
          </button>
          {cookResult && (
            <span className="mesh-stats">
              {cookResult.smoke_particle_count != null && cookResult.smoke_particle_count > 0
                ? `${cookResult.smoke_particle_count} particles · ${cookResult.smoke_steps} steps`
                : `${cookResult.instance_count} inst · ${cookResult.vertex_count}v · ${cookResult.triangle_count}t`}
            </span>
          )}
          {exportResult && (
            <span className="export-path" title={exportResult.path}>
              glTF exported
            </span>
          )}
          {smokeExportResult && (
            <span className="export-path" title={smokeExportResult.path}>
              smoke exported
            </span>
          )}
        </div>
      </footer>
    </div>
  );
}

export default App;
