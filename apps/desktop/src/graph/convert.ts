import type { Edge, Node } from "@xyflow/react";
import type { ElfNodeData } from "../components/nodes/ElfNode";
import type { Graph, GraphEdge, GraphNode } from "../types/graph";

const PRESET_LAYOUT: Record<string, { x: number; y: number }> = {
  building_params: { x: 40, y: 120 },
  building_mesh: { x: 280, y: 120 },
  place_along_path: { x: 520, y: 120 },
  fill_grid: { x: 520, y: 280 },
  merge_instances: { x: 520, y: 400 },
  city_root: { x: 760, y: 120 },
  smoke_domain: { x: 40, y: 140 },
  smoke_source: { x: 280, y: 140 },
  smoke_solver: { x: 520, y: 140 },
  smoke_root: { x: 760, y: 140 },
  liquid_domain: { x: 40, y: 140 },
  liquid_source: { x: 280, y: 140 },
  liquid_solver: { x: 520, y: 140 },
  liquid_root: { x: 760, y: 140 },
};

export function graphToFlow(graph: Graph): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = graph.nodes.map((n) => {
    const layout = PRESET_LAYOUT[n.id] ?? { x: 100, y: 100 };
    return {
      id: n.id,
      type: "elfNode",
      position: layout,
      data: {
        kind: n.kind,
        label: n.label,
        buildingParams: n.building_params ?? undefined,
      } satisfies ElfNodeData,
    };
  });

  const edges: Edge[] = graph.edges.map((e, i) => ({
    id: `e-${i}`,
    source: e.from,
    target: e.to,
    animated: true,
    style: { stroke: "rgba(110,168,255,0.55)" },
  }));

  return { nodes, edges };
}

export function flowToGraph(
  nodes: Node[],
  edges: Edge[],
  name: string,
): Graph {
  const graphNodes: GraphNode[] = nodes.map((n) => {
    const data = n.data as ElfNodeData;
    return {
      id: n.id,
      kind: data.kind,
      label: data.label,
      building_params: data.buildingParams ?? null,
      path_input: null,
      grid_input: null,
      smoke_domain: null,
      smoke_source: null,
      smoke_solver: null,
      liquid_domain: null,
      liquid_source: null,
      liquid_solver: null,
    };
  });

  const graphEdges: GraphEdge[] = edges.map((e) => ({
    from: e.source,
    to: e.target,
  }));

  return { name, nodes: graphNodes, edges: graphEdges };
}

export function mergePresetInputs(graph: Graph, preset: Graph): Graph {
  return {
    ...graph,
    nodes: graph.nodes.map((node) => {
      const presetNode = preset.nodes.find((n) => n.id === node.id);
      if (!presetNode) return node;
      return {
        ...node,
        path_input: presetNode.path_input,
        grid_input: presetNode.grid_input,
        building_params: node.building_params ?? presetNode.building_params,
        smoke_domain: node.smoke_domain ?? presetNode.smoke_domain,
        smoke_source: node.smoke_source ?? presetNode.smoke_source,
        smoke_solver: node.smoke_solver ?? presetNode.smoke_solver,
        liquid_domain: node.liquid_domain ?? presetNode.liquid_domain,
        liquid_source: node.liquid_source ?? presetNode.liquid_source,
        liquid_solver: node.liquid_solver ?? presetNode.liquid_solver,
      };
    }),
  };
}
