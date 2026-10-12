import { Handle, type NodeProps, Position } from "@xyflow/react";
import type { NodeStatus } from "../bindings";
import { STATUS_SIGN } from "./nodes";
import type { Box } from "./tree";

export interface BoxData extends Record<string, unknown> {
  box: Box;
  /** Has boxes inside to open. */
  expandable: boolean;
  /** A symbol's status, or a group's own symbol's. Absent for folders and files. */
  status?: NodeStatus;
  /** Every symbol in it is an unchanged caller. */
  outside: boolean;
  /** Changed symbols in a closed folder, file or group. */
  count: number;
  /** Share of its changed rows that are reviewed or skipped. */
  done?: number;
  /** The worst hotspot level on its rows. */
  risk?: string;
  /** Callers one hop out from its own symbols, and whether they're shown. */
  callers: number;
  callersOpen: boolean;
  onCallers(): void;
  title: string;
}

function CallersBadge({ data }: { data: BoxData }) {
  if (data.callers === 0) return null;
  return (
    <button
      type="button"
      className={`gcallers${data.callersOpen ? " open" : ""}`}
      title={data.callersOpen ? "Hide callers" : "Show callers"}
      onClick={(e) => {
        e.stopPropagation();
        data.onCallers();
      }}
    >
      {data.callersOpen ? "−" : "+"}
      {data.callers} {data.callers === 1 ? "caller" : "callers"}
    </button>
  );
}

const chevron = (data: BoxData, open: boolean) =>
  data.expandable ? <span className="gchevron">{open ? "▾" : "▸"}</span> : null;

/** A symbol, or a closed folder, file or group, with a bar for how much of it is reviewed. */
export function BoxNode({ data }: NodeProps & { data: BoxData }) {
  const { box, status, done, risk, count } = data;
  const kind = status ?? (data.outside ? "caller" : "box");
  return (
    <div className={`gnode gnode-${kind}${risk ? ` risk-${risk}` : ""}`} title={data.title}>
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <div className="gnode-name">
        {chevron(data, false)}
        {status && <span className="gnode-sign">{STATUS_SIGN[status]}</span>}
        {box.label}
        {box.kind !== "symbol" && count > 0 && <span className="gcount">{count}</span>}
        <CallersBadge data={data} />
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

/** An open folder, file or group: a box around what's in it. */
export function OpenBoxNode({ data }: NodeProps & { data: BoxData }) {
  const { box, status, risk } = data;
  return (
    <div
      className={`gfile gfile-${box.kind}${data.outside ? " outside" : ""}${risk ? ` risk-${risk}` : ""}`}
      title={data.title}
    >
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <span className="gfile-head">
        {chevron(data, true)}
        {status && <span className="gnode-sign">{STATUS_SIGN[status]}</span>}
        {box.label}
        <CallersBadge data={data} />
      </span>
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
}
