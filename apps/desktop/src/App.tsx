import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import NodeCanvas from "./components/NodeCanvas";
import "./App.css";

interface MeshStats {
  vertex_count: number;
  index_count: number;
  triangle_count: number;
}

function App() {
  const [coreVersion, setCoreVersion] = useState("…");
  const [meshStats, setMeshStats] = useState<MeshStats | null>(null);
  const [status, setStatus] = useState("Starting…");

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      try {
        const version = await invoke<string>("get_core_version");
        if (cancelled) return;
        setCoreVersion(version);
        setStatus(`Core ${version} ready`);
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

  const handleCreateBox = useCallback(async () => {
    setStatus("Creating unit box mesh…");
    try {
      const stats = await invoke<MeshStats>("create_box_mesh");
      setMeshStats(stats);
      setStatus(
        `Box mesh: ${stats.vertex_count} verts · ${stats.triangle_count} tris · ${stats.index_count} indices`,
      );
    } catch (error) {
      setStatus(`Mesh command failed: ${String(error)}`);
    }
  }, []);

  return (
    <div className="app">
      <header className="app-header">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">◆</span>
          <h1>elfentierFX</h1>
          <span className="brand-tag">Alpha 0</span>
        </div>
        <p className="brand-subtitle">Unity-oriented procedural DCC</p>
      </header>

      <main className="app-main">
        <NodeCanvas />
      </main>

      <footer className="app-footer">
        <span className="status-line">{status}</span>
        <div className="footer-actions">
          <span className="core-version">core {coreVersion}</span>
          <button type="button" className="action-button" onClick={handleCreateBox}>
            Create unit box
          </button>
          {meshStats && (
            <span className="mesh-stats">
              {meshStats.vertex_count}v / {meshStats.triangle_count}t
            </span>
          )}
        </div>
      </footer>
    </div>
  );
}

export default App;
