import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Background,
  Controls,
  ReactFlow,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  type Edge,
  type Node,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { flowToGraph, graphToFlow, mergePresetInputs } from "../graph/convert";
import type { BuildingParams, Graph } from "../types/graph";
import ElfNode from "./nodes/ElfNode";
import "./NodeCanvas.css";

const nodeTypes = { elfNode: ElfNode };

interface NodeCanvasProps {
  preset: Graph | null;
  graphName: string;
  onGraphChange: (graph: Graph) => void;
}

function NodeCanvasInner({ preset, graphName, onGraphChange }: NodeCanvasProps) {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [basePreset, setBasePreset] = useState<Graph | null>(null);

  const attachParamHandlers = useCallback(
    (flowNodes: Node[], currentEdges: typeof edges) => {
      return flowNodes.map((node) => {
        const data = node.data as { kind: string; buildingParams?: BuildingParams };
        if (data.kind !== "building_params" || !data.buildingParams) return node;
        return {
          ...node,
          data: {
            ...data,
            onParamsChange: (params: BuildingParams) => {
              setNodes((nds) => {
                const updated = nds.map((n) =>
                  n.id === node.id
                    ? { ...n, data: { ...n.data, buildingParams: params } }
                    : n,
                );
                const g = mergePresetInputs(
                  flowToGraph(updated, currentEdges, graphName),
                  basePreset ?? { name: graphName, nodes: [], edges: [] },
                );
                onGraphChange(g);
                return updated;
              });
            },
          },
        };
      });
    },
    [basePreset, graphName, onGraphChange, setNodes],
  );

  useEffect(() => {
    if (!preset) return;
    setBasePreset(preset);
    const { nodes: flowNodes, edges: flowEdges } = graphToFlow(preset);
    const withHandlers = attachParamHandlers(flowNodes, flowEdges);
    setNodes(withHandlers);
    setEdges(flowEdges);
    onGraphChange(preset);
  }, [preset, attachParamHandlers, onGraphChange, setEdges, setNodes]);

  useEffect(() => {
    if (nodes.length === 0) return;
    const g = mergePresetInputs(
      flowToGraph(nodes, edges, graphName),
      basePreset ?? { name: graphName, nodes: [], edges: [] },
    );
    onGraphChange(g);
  }, [nodes, edges, graphName, basePreset, onGraphChange]);

  const proOptions = useMemo(() => ({ hideAttribution: true }), []);

  return (
    <div className="node-canvas">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        nodeTypes={nodeTypes}
        fitView
        fitViewOptions={{ padding: 0.2 }}
        proOptions={proOptions}
      >
        <Background gap={20} size={1} color="rgba(255,255,255,0.04)" />
        <Controls showInteractive={false} />
      </ReactFlow>
    </div>
  );
}

export default function NodeCanvas(props: NodeCanvasProps) {
  return (
    <ReactFlowProvider>
      <NodeCanvasInner {...props} />
    </ReactFlowProvider>
  );
}
