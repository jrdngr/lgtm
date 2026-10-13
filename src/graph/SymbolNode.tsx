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
  /** The worst level of the important notes on its rows. */
  risk?: string;
  /** Callers one hop out from its own symbols, and whether they're shown. */
  callers: number;
  callersOpen: boolean;
  onCallers(): void;
  title: string;
}

/** "2 callers" in dim text, a chip on hover or while they're shown. */
function CallersBadge({ data }: { data: BoxData }) {
  if (data.callers === 0) return null;
  return (
    <button
      type="button"
      className={`gcallers${data.callersOpen ? " open" : ""}`}
      title={data.callersOpen ? "Hide callers" : "Show callers"}
      aria-pressed={data.callersOpen}
      onClick={(e) => {
        e.stopPropagation();
        data.onCallers();
      }}
    >
      {data.callers} {data.callers === 1 ? "caller" : "callers"}
    </button>
  );
}

const chevron = (data: BoxData, open: boolean) =>
  data.expandable ? <span className="gchevron">{open ? "▾" : "▸"}</span> : null;

const sign = (status?: NodeStatus) =>
  status && status !== "caller" ? (
    <span className={`gsign gsign-${status}`}>{STATUS_SIGN[status]}</span>
  ) : null;

/** A dot in the corner for the worst important note in it. */
const riskDot = (risk?: string) =>
  risk ? <span className={`gdot risk-${risk}`} title={`Important note, ${risk}`} /> : null;

/** A symbol, or a closed folder, file or group, with a line along its bottom for how much of
 * it is reviewed. */
export function BoxNode({ data, selected }: NodeProps & { data: BoxData }) {
  const { box, status, done, risk, count } = data;
  const kind = status ?? (data.outside ? "caller" : `box gnode-${box.kind}`);
  return (
    <div className={`gnode gnode-${kind}${selected ? " selected" : ""}`} title={data.title}>
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <div className="gnode-name">
        {chevron(data, false)}
        {sign(status)}
        <span className="glabel">{box.label}</span>
        {box.kind !== "symbol" && count > 0 && <span className="gcount">{count}</span>}
        <CallersBadge data={data} />
      </div>
      {riskDot(risk)}
      {done !== undefined && (
        <div className="gnode-bar">
          <span style={{ width: `${Math.round(done * 100)}%` }} />
        </div>
      )}
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
}

/** An open folder, file or group: a box around what's in it, under a header band. */
export function OpenBoxNode({ data, selected }: NodeProps & { data: BoxData }) {
  const { box, status, risk } = data;
  return (
    <div
      className={`gfile gfile-${box.kind}${data.outside ? " outside" : ""}${selected ? " selected" : ""}`}
      title={data.title}
    >
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <div className="gfile-head">
        {chevron(data, true)}
        {sign(status)}
        <span className="glabel">{box.label}</span>
        <CallersBadge data={data} />
      </div>
      {riskDot(risk)}
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
}
