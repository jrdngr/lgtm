import { Handle, type NodeProps, Position } from "@xyflow/react";
import type { GraphNode } from "../bindings";
import { nodeLabel, STATUS_SIGN } from "./nodes";

export interface SymbolData extends Record<string, unknown> {
  node: GraphNode;
  /** Share of its changed rows that are reviewed or skipped. */
  done?: number;
  /** The worst hotspot level on its rows. */
  risk?: string;
}

/** A symbol in the graph, with a bar for how much of it is reviewed. */
export function SymbolNode({ data }: NodeProps & { data: SymbolData }) {
  const { node, done, risk } = data;
  const title =
    node.status === "caller"
      ? `${node.path}:${node.line}, unchanged, uses a changed symbol`
      : `${node.path}:${node.line}, ${node.status}, ${node.rows.length} changed rows`;
  return (
    <div className={`gnode gnode-${node.status}${risk ? ` risk-${risk}` : ""}`} title={title}>
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <div className="gnode-name">
        <span className="gnode-sign">{STATUS_SIGN[node.status]}</span>
        {nodeLabel(node)}
      </div>
      {done !== undefined && (
        <div className="gnode-bar">
          <span style={{ width: `${Math.round(done * 100)}%` }} />
        </div>
      )}
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
}

/** A file's box around its symbols. */
export function FileNode({ data }: NodeProps & { data: { path: string; outside: boolean } }) {
  return (
    <div className={`gfile${data.outside ? " outside" : ""}`} title={data.path}>
      <span>{data.path}</span>
    </div>
  );
}
