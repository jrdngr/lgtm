import type { NodeProps } from "@xyflow/react";
import type { DragEvent } from "react";
import type { ComponentDto, LookSeverity } from "../bindings";
import type { CurrentReview } from "../review/current";
import { NODE_H, NODE_W } from "./layout";

/** What a box in the graph shows: a component, or a container at the top level. */
export interface NodeView extends Record<string, unknown> {
  id: string;
  name: string;
  prefix?: "+" | "~" | "−";
  /** Top right, like "3 files". */
  count?: string;
  /** The second line, like "impacted". */
  sub?: { text: string; tone?: "amber" | "violet" };
  /** Share of its changed rows that are done. */
  progress?: number;
  /** `up`, `imp`, `rem` and `misplaced`, for how the box draws. */
  kind: string;
  risk: boolean;
  layer?: number;
  severity?: LookSeverity | null;
  /** In focus, out of it, or no focus. */
  focus?: "sel" | "dimn";
  /** Takes a dragged file or symbol. */
  onDrop?(e: DragEvent): void;
}

const PREFIX = { added: "+", changed: "~", removed: "−" } as const;
const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;

/** Share of the component's changed rows that are done, if the review changes any. */
function progressOf(review: CurrentReview, c: ComponentDto): number | undefined {
  let done = 0;
  let all = 0;
  for (const f of c.files) {
    const p = f.file === null ? undefined : review.progress[f.file];
    done += p?.done ?? 0;
    all += p?.changed ?? 0;
  }
  return all === 0 ? undefined : done / all;
}

export function componentView(review: CurrentReview, c: ComponentDto): NodeView {
  const prefix = c.status in PREFIX ? PREFIX[c.status as keyof typeof PREFIX] : undefined;
  const changedFiles = c.files.filter((f) => f.file !== null).length;
  const misplaced = c.misplaced.length;
  const sub =
    misplaced > 0
      ? { text: `${misplaced} misplaced`, tone: "amber" as const }
      : c.status === "impacted"
        ? { text: "impacted", tone: "violet" as const }
        : c.status === "upstream"
          ? { text: "upstream" }
          : undefined;
  const kind = {
    upstream: "up",
    impacted: "imp",
    removed: "rem",
    added: "",
    changed: "",
    unchanged: "",
  }[c.status];
  return {
    id: c.id,
    name: c.name,
    prefix,
    count: prefix ? plural(changedFiles, "file") : undefined,
    sub,
    progress: prefix && misplaced === 0 ? (progressOf(review, c) ?? 0) : undefined,
    kind: `${kind}${misplaced > 0 ? " misplaced" : ""}`,
    risk: c.risk,
    layer: c.layer ?? undefined,
    severity: c.severity,
  };
}

/** A container at the top level, standing for its components. */
export function containerView(id: string, name: string, comps: ComponentDto[]): NodeView {
  const changed = comps.filter((c) => c.status in PREFIX).length;
  return {
    id,
    name,
    prefix: changed > 0 ? "~" : undefined,
    sub: {
      text: `${changed > 0 ? `${changed} changed · ` : ""}${plural(comps.length, "component")}`,
    },
    kind: comps.some((c) => c.status === "impacted") && changed === 0 ? "imp" : "",
    risk: comps.some((c) => c.risk),
  };
}

/** A component box, as the mockup draws it: name with its change sign, files count, a second
 * line for upstream, impacted or misplaced, review progress along the bottom, its layer
 * number on the top left corner and its worst Look item's dot on the top right. */
export function ArchNode({ data, selected }: NodeProps & { data: NodeView }) {
  const v = data;
  const focus = selected ? "sel" : v.focus;
  return (
    <div
      className={`anode ${v.kind}${focus ? ` ${focus}` : ""}`}
      style={{ width: NODE_W, height: NODE_H }}
      onDragOver={
        v.onDrop &&
        ((e) => {
          if (!e.dataTransfer.types.includes(DRAG_TYPE)) return;
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
        })
      }
      onDrop={v.onDrop}
    >
      {v.risk && <span className="arisk" />}
      <div className="aline">
        {v.prefix && (
          <span className={`apre ${v.prefix === "+" ? "add" : v.prefix === "~" ? "mod" : "rem"}`}>
            {v.prefix}
          </span>
        )}
        <span className="aname">{v.name}</span>
        {v.count && <span className="acount">{v.count}</span>}
      </div>
      {v.sub && <div className={`asub${v.sub.tone ? ` ${v.sub.tone}` : ""}`}>{v.sub.text}</div>}
      {v.progress !== undefined && (
        <div className="aprog">
          <span style={{ width: `${Math.round(v.progress * 100)}%` }} />
        </div>
      )}
      {v.layer !== undefined && <span className="alayer">{v.layer}</span>}
      {v.severity && <span className={`asev ${v.severity}`} />}
    </div>
  );
}

/** The drag data type of a file or symbol on its way to another component. */
export const DRAG_TYPE = "application/x-lgtm-arch-item";

export interface DragItem {
  path: string;
  symbol: string | null;
}

export function startItemDrag(e: DragEvent, item: DragItem) {
  e.dataTransfer.effectAllowed = "move";
  e.dataTransfer.setData(DRAG_TYPE, JSON.stringify(item));
}

export function droppedItem(e: DragEvent): DragItem | undefined {
  const raw = e.dataTransfer.getData(DRAG_TYPE);
  if (!raw) return undefined;
  e.preventDefault();
  return JSON.parse(raw) as DragItem;
}
