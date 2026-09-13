import { Background, ReactFlow, ReactFlowProvider } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import "./NodeCanvas.css";

function NodeCanvasInner() {
  return (
    <div className="node-canvas">
      <ReactFlow
        nodes={[]}
        edges={[]}
        fitView
        proOptions={{ hideAttribution: true }}
      >
        <Background gap={20} size={1} color="rgba(255,255,255,0.04)" />
      </ReactFlow>
      <div className="node-canvas-overlay">
        <p className="node-canvas-title">Node graph</p>
        <p className="node-canvas-hint">Alpha 0 placeholder — procedural nodes land in Alpha 1</p>
      </div>
    </div>
  );
}

export default function NodeCanvas() {
  return (
    <ReactFlowProvider>
      <NodeCanvasInner />
    </ReactFlowProvider>
  );
}
