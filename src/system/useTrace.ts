import { useMemo, useState } from "react";
import type { SystemDto } from "../bindings";
import { tracePath, tracePaths } from "./trace";

export type End = "from" | "to";
/** The shortest path drawn straight, all of them, or the shortest over the map. */
export type TraceShow = "shortest" | "all" | "map";

export interface Trace {
  from?: string;
  to?: string;
  /** The end being picked. */
  active?: End;
  show: TraceShow;
}

const other = (e: End): End => (e === "from" ? "to" : "from");
const unset = (t: { from?: string; to?: string }): End | undefined =>
  !t.from ? "from" : !t.to ? "to" : undefined;

/** A trace between two nodes: its ends, the end being picked, and the paths between them. */
export function useTrace(map: SystemDto) {
  const [trace, setTrace] = useState<Trace>();
  const from = trace?.from;
  const to = trace?.to;
  /** Undefined while an end is unpicked; empty when nothing joins them. */
  const hops = useMemo(
    () => (from && to ? (tracePath(map, from, to) ?? []) : undefined),
    [map, from, to],
  );
  const all = useMemo(() => (from && to ? tracePaths(map, from, to) : []), [map, from, to]);

  /** Starts with any ends known, picking the first unset one. */
  const start = (ends: { from?: string; to?: string }) =>
    setTrace({ ...ends, active: unset(ends), show: "shortest" });
  /** Sets `end`, or the end being picked, or the first unset one, then picks the other end if
   * it's unset. Returns the trace it makes. */
  const pick = (id: string, end?: End) => {
    const e = end ?? trace?.active ?? (trace && unset(trace));
    if (!trace || !e) return trace;
    const next: Trace = { ...trace, [e]: id };
    next.active = next[other(e)] ? undefined : other(e);
    setTrace(next);
    return next;
  };
  const edit = (end: End) => trace && setTrace({ ...trace, active: end });
  const swap = () =>
    trace &&
    setTrace({
      ...trace,
      from: trace.to,
      to: trace.from,
      active: trace.active && other(trace.active),
    });
  const show = (s: TraceShow) => trace && setTrace({ ...trace, show: s });
  const end = () => setTrace(undefined);
  return { trace, hops, all, start, pick, edit, swap, show, end };
}
