import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import NodeCanvas from "./components/NodeCanvas";
import type { CookResult, ExportResult, Graph } from "./types/graph";
import "./App.css";

function App() {
  const [coreVersion, setCoreVersion] = useState("…");
  const [status, setStatus] = useState("Starting…");
  const [preset, setPreset] = useState<Graph | null>(null);
  const [graph, setGraph] = useState<Graph | null>(null);
  const [cookResult, setCookResult] = useState<CookResult | null>(null);
  const [exportResult, setExportResult] = useState<ExportResult | null>(null);
  const graphRef = useRef<Graph | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      try {
        const [version, shopPreset] = await Promise.all([
          invoke<string>("get_core_version"),
          invoke<Graph>("get_shop_street_preset"),
        ]);
        if (cancelled) return;
        setCoreVersion(version);
        setPreset(shopPreset);
        setGraph(shopPreset);
        graphRef.current = shopPreset;
        setStatus(`Core ${version} ready — shop street preset loaded`);
      } catch (error) {
        if (cancelled) return;
        setStatus(`Core unavailable: ${String(error)}`);
      }
    }

    bootstrap();
    return () => {
      cancelled = true;
    };
  }, []);

  const handleGraphChange = useCallback((next: Graph) => {
    graphRef.current = next;
    setGraph(next);
  }, []);

  const handleLoadPreset = useCallback(async () => {
    setStatus("Loading shop street preset…");
    try {
      const shopPreset = await invoke<Graph>("get_shop_street_preset");
      setPreset(shopPreset);
      setGraph(shopPreset);
      graphRef.current = shopPreset;
      setCookResult(null);
      setExportResult(null);
      setStatus("Shop street preset loaded");
    } catch (error) {
      setStatus(`Preset failed: ${String(error)}`);
    }
  }, []);

  const handleCook = useCallback(async () => {
    const current = graphRef.current;
    if (!current) {
      setStatus("No graph to cook");
      return;
    }
    setStatus("Cooking city graph…");
    try {
      const result = await invoke<CookResult>("cook_city_graph", { graph: current });
      setCookResult(result);
      setStatus(
        `Cooked "${result.graph_name}": ${result.instance_count} instances · ${result.vertex_count} verts · ${result.triangle_count} tris`,
      );
    } catch (error) {
      setStatus(`Cook failed: ${String(error)}`);
    }
  }, []);

  const handleExport = useCallback(async () => {
    const current = graphRef.current;
    if (!current) {
      setStatus("No graph to export");
      return;
    }
    setStatus("Exporting glTF…");
    try {
      const path = "/tmp/elfentier_city.glb";
      const result = await invoke<ExportResult>("export_city_graph", {
        graph: current,
        path,
      });
      setExportResult(result);
      setStatus(
        `Exported ${result.byte_len} bytes → ${result.path} (${result.triangle_count} tris)`,
      );
    } catch (error) {
      setStatus(`Export failed: ${String(error)}`);
    }
  }, []);

  return (
    <div className="app">
      <header className="app-header">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">◆</span>
          <h1>elfentierFX</h1>
          <span className="brand-tag">Alpha 1</span>
        </div>
        <p className="brand-subtitle">Building → city procedural slice</p>
      </header>

      <main className="app-main">
        <NodeCanvas
          preset={preset}
          graphName={graph?.name ?? "City Graph"}
          onGraphChange={handleGraphChange}
        />
      </main>

      <footer className="app-footer">
        <span className="status-line">{status}</span>
        <div className="footer-actions">
          <span className="core-version">core {coreVersion}</span>
          <button type="button" className="action-button secondary" onClick={handleLoadPreset}>
            Shop → Street preset
          </button>
          <button type="button" className="action-button" onClick={handleCook}>
            Cook
          </button>
          <button type="button" className="action-button" onClick={handleExport}>
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
