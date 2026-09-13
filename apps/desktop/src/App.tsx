import { useCallback, useEffect, useRef, useState } from "react";
import { agentApi } from "./agent/api";
import NodeCanvas from "./components/NodeCanvas";
import PromptBar from "./components/PromptBar";
import SplitPane from "./components/SplitPane";
import type {
  CookResult,
  ExportResult,
  Graph,
  SmokePreviewImage,
  SmokeVolumeExport,
  ViewportCook,
} from "./types/graph";
import Viewport3D from "./viewport/Viewport3D";
import "./App.css";

function App() {
  const [coreVersion, setCoreVersion] = useState("…");
  const [status, setStatus] = useState("Starting…");
  const [preset, setPreset] = useState<Graph | null>(null);
  const [graph, setGraph] = useState<Graph | null>(null);
  const [cookResult, setCookResult] = useState<CookResult | null>(null);
  const [viewport, setViewport] = useState<ViewportCook | null>(null);
  const [smokePreview, setSmokePreview] = useState<SmokePreviewImage | null>(null);
  const [exportResult, setExportResult] = useState<ExportResult | null>(null);
  const [smokeExport, setSmokeExport] = useState<SmokeVolumeExport | null>(null);
  const [promptSummary, setPromptSummary] = useState("");
  const [explainText, setExplainText] = useState("");
  const [busy, setBusy] = useState(false);
  const graphRef = useRef<Graph | null>(null);

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

  const loadPresetGraph = useCallback(
    async (loader: () => Promise<Graph>, label: string) => {
      setBusy(true);
      setStatus(`Loading ${label}…`);
      try {
        const next = await loader();
        setPreset(next);
        setGraph(next);
        graphRef.current = next;
        setCookResult(null);
        setViewport(null);
        setSmokePreview(null);
        setExportResult(null);
        setSmokeExport(null);
        setPromptSummary("");
        setStatus(`${label} loaded`);
        refreshExplain(next);
      } catch (error) {
        setStatus(`Preset failed: ${String(error)}`);
      } finally {
        setBusy(false);
      }
    },
    [refreshExplain],
  );

  const handleLoadShopPreset = useCallback(
    () => loadPresetGraph(agentApi.getShopStreetPreset, "Shop street preset"),
    [loadPresetGraph],
  );

  const handleLoadSmokePreset = useCallback(
    () => loadPresetGraph(agentApi.getSmokePlumePreset, "Smoke plume preset"),
    [loadPresetGraph],
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
      setViewport(result.viewport);
      setSmokePreview(result.smoke_preview);
      if (result.stats.output_kind === "smoke") {
        setStatus(
          `Cooked "${result.stats.graph_name}": ${result.stats.instance_count} voxels · max density ${result.viewport.smoke?.max_density.toFixed(2) ?? "?"}`,
        );
      } else {
        setStatus(
          `Cooked "${result.stats.graph_name}": ${result.stats.instance_count} instances · ${result.stats.vertex_count} verts · ${result.stats.triangle_count} tris`,
        );
      }
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
    const isSmoke = current.nodes.some((n) => n.kind === "smoke_root");
    setStatus(isSmoke ? "Exporting smoke volume…" : "Exporting glTF…");
    try {
      if (isSmoke) {
        const path = "/tmp/elfentier_smoke.raw";
        const result = await agentApi.exportSmokeVolume(current, path);
        setSmokeExport(result);
        setExportResult(null);
        setStatus(`Exported ${result.byte_len} bytes → ${result.path} (${result.format})`);
      } else {
        const path = "/tmp/elfentier_city.glb";
        const result = await agentApi.exportGltf(current, path);
        setExportResult(result);
        setSmokeExport(null);
        setStatus(
          `Exported ${result.byte_len} bytes → ${result.path} (${result.triangle_count} tris)`,
        );
      }
    } catch (error) {
      setStatus(`Export failed: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }, []);

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
        setViewport(null);
        setSmokePreview(null);
        setExportResult(null);
        setSmokeExport(null);
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

  const isSmokeGraph = graph?.nodes.some((n) => n.kind === "smoke_root") ?? false;

  return (
    <div className="app">
      <header className="app-header">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">◆</span>
          <h1>elfentierFX</h1>
          <span className="brand-tag">Alpha 3</span>
        </div>
        <p className="brand-subtitle">
          Building → city · Fluids smoke Phase 1 · native wgpu preview · prompt-driven graph
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
          right={<Viewport3D viewport={viewport} smokePreview={smokePreview} />}
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
            Smoke plume
          </button>
          <button type="button" className="action-button" onClick={handleCook} disabled={busy}>
            Cook
          </button>
          <button type="button" className="action-button" onClick={handleExport} disabled={busy}>
            {isSmokeGraph ? "Export volume" : "Export glTF"}
          </button>
          {cookResult && cookResult.output_kind === "smoke" && (
            <span className="mesh-stats">
              {cookResult.instance_count} voxels
            </span>
          )}
          {cookResult && cookResult.output_kind !== "smoke" && (
            <span className="mesh-stats">
              {cookResult.instance_count} inst · {cookResult.vertex_count}v · {cookResult.triangle_count}t
            </span>
          )}
          {exportResult && (
            <span className="export-path" title={exportResult.path}>
              glb exported
            </span>
          )}
          {smokeExport && (
            <span className="export-path" title={smokeExport.notes}>
              volume exported
            </span>
          )}
        </div>
      </footer>
    </div>
  );
}

export default App;
