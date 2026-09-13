import { Handle, Position, type NodeProps } from "@xyflow/react";
import {
  NODE_KIND_COLORS,
  NODE_KIND_LABELS,
  type BuildingParams,
  type NodeKind,
} from "../../types/graph";

export interface ElfNodeData {
  kind: NodeKind;
  label: string;
  buildingParams?: BuildingParams;
  onParamsChange?: (params: BuildingParams) => void;
  [key: string]: unknown;
}

export default function ElfNode({ data, selected }: NodeProps) {
  const nodeData = data as ElfNodeData;
  const color = NODE_KIND_COLORS[nodeData.kind];
  const kindLabel = NODE_KIND_LABELS[nodeData.kind];
  const isParams = nodeData.kind === "building_params" && nodeData.buildingParams;

  return (
    <div className={`elf-node ${selected ? "elf-node--selected" : ""}`} style={{ borderColor: color }}>
      <div className="elf-node-header" style={{ background: `${color}22` }}>
        <span className="elf-node-kind" style={{ color }}>{kindLabel}</span>
        <span className="elf-node-label">{nodeData.label}</span>
      </div>

      {isParams && nodeData.buildingParams && (
        <div className="elf-node-params">
          <label className="elf-node-field">
            <span>Floors</span>
            <input
              type="number"
              min={1}
              max={20}
              value={nodeData.buildingParams.floors}
              onChange={(e) =>
                nodeData.onParamsChange?.({
                  ...nodeData.buildingParams!,
                  floors: Number(e.target.value),
                })
              }
            />
          </label>
          <label className="elf-node-field">
            <span>Seed</span>
            <input
              type="number"
              min={0}
              value={nodeData.buildingParams.seed}
              onChange={(e) =>
                nodeData.onParamsChange?.({
                  ...nodeData.buildingParams!,
                  seed: Number(e.target.value),
                })
              }
            />
          </label>
          <label className="elf-node-field">
            <span>Window density</span>
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={nodeData.buildingParams.window_density}
              onChange={(e) =>
                nodeData.onParamsChange?.({
                  ...nodeData.buildingParams!,
                  window_density: Number(e.target.value),
                })
              }
            />
          </label>
        </div>
      )}

      {nodeData.kind !== "building_params" && (
        <p className="elf-node-hint">
          {nodeData.kind === "building_mesh" && "Generates mesh from params"}
          {nodeData.kind === "place_along_path" && "Instances along street path"}
          {nodeData.kind === "fill_grid" && "Fills lot grid with instances"}
          {nodeData.kind === "merge_instances" && "Combines instance lists"}
          {nodeData.kind === "city_root" && "Final city output"}
        </p>
      )}

      {nodeData.kind !== "building_params" && (
        <Handle type="target" position={Position.Left} className="elf-handle" />
      )}
      {nodeData.kind !== "city_root" && (
        <Handle type="source" position={Position.Right} className="elf-handle" />
      )}
    </div>
  );
}
