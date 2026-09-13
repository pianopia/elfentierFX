import { useCallback, useEffect, useRef, useState } from "react";
import { agentApi } from "./agent/api";
import NodeCanvas from "./components/NodeCanvas";
import PromptBar from "./components/PromptBar";
import SplitPane from "./components/SplitPane";
import type {
  CookResult,
  ExportResult,
  Graph,
  ViewportMesh,
} from "./types/graph";
import Viewport3D from "./viewport/Viewport3D";
import "./App.css";

function App() {
  const [coreVersion, setCoreVersion] = useState("…");
  const [status, setStatus] = useState("Starting…");
  const [preset, setPreset] = useState<Graph | null>(null);
  const [graph, setGraph] = useState<Graph | null>(null);
  const [cookResult, setCookResult] = useState<CookResult | null>(null);
  const [viewportMesh, setViewportMesh] = useState<ViewportMesh | null>(null);
  const [exportResult, setExportResult] = useState<ExportResult | null>(null);
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

  const handleLoadPreset = useCallback(async () => {
    setBusy(true);
    setStatus("Loading shop street preset…");
    try {
      const shopPreset = await agentApi.getShopStreetPreset();
      setPreset(shopPreset);
      setGraph(shopPreset);
      graphRef.current = shopPreset;
      setCookResult(null);
      setViewportMesh(null);
      setExportResult(null);
      setPromptSummary("");
      setStatus("Shop street preset loaded");
      refreshExplain(shopPreset);
    } catch (error) {
      setStatus(`Preset failed: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }, [refreshExplain]);

  const handleCook = useCallback(async () => {
    const current = graphRef.current;
    if (!current) {
      setStatus("No graph to cook");
      return;
    }
    setBusy(true);
    setStatus("Cooking city graph…");
    try {
      const result = await agentApi.cook(current);
      setCookResult(result.stats);
      setViewportMesh(result.mesh);
      setStatus(
        `Cooked "${result.stats.graph_name}": ${result.stats.instance_count} instances · ${result.stats.vertex_count} verts · ${result.stats.triangle_count} tris`,
      );
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
        setViewportMesh(null);
        setExportResult(null);
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
        <p className="brand-subtitle">Building → city procedural slice · 3D viewport · prompt-driven graph</p>
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
              graphName={graph?.name ?? "City Graph"}
              onGraphChange={handleGraphChange}
            />
          }
          right={<Viewport3D mesh={viewportMesh} />}
        />
      </main>

      <footer className="app-footer">
        <span className="status-line">{status}</span>
        <div className="footer-actions">
          <span className="core-version">core {coreVersion}</span>
          <button
            type="button"
            className="action-button secondary"
            onClick={handleLoadPreset}
            disabled={busy}
          >
            Shop → Street preset
          </button>
          <button type="button" className="action-button" onClick={handleCook} disabled={busy}>
            Cook
          </button>
          <button type="button" className="action-button" onClick={handleExport} disabled={busy}>
            Export glTF
          </button>
          {cookResult && (
            <span className="mesh-stats">
              {cookResult.instance_count} inst · {cookResult.vertex_count}v · {cookResult.triangle_count}t
            </span>
          )}
          {exportResult && (
            <span className="export-path" title={exportResult.path}>
              exported
            </span>
          )}
        </div>
      </footer>
    </div>
  );
}

export default App;
