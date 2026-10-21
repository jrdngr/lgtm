import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import type {
  ArrowEnd,
  CanvasArrow,
  CanvasNote,
  EdgeStyle,
  NoteSize,
  Swatch,
} from "../../bindings";
import { boxExit, type Pt } from "../../system/edgePaths";
import type { Box, Layout } from "../../system/layout";
import { BLUE, DASHED, DOTTED } from "../../system/MapEdges";
import type { Paint } from "../../system/mapWires";
import type { ToWorld } from "../../system/SystemCanvas";
import type { SEdge } from "../../system/scene";
import { type Arranged, arrange, freeze } from "./arrange";
import {
  type GroupBox,
  group,
  groupAt,
  groupBoxes,
  groupsIn,
  moveInto,
  parentOf,
  partsIn,
  ungroup,
} from "./canvasGroups";
import { noteBox } from "./notes";
import { swatchHex } from "./palette";
import { arrowRef, edgeRef, groupRef, idsOf, noteRef, parseRef } from "./refs";
import { type Canvas, freshId, isPart, type View, viewOf, withView } from "./view";

export type Tool = "select" | "text" | "arrow";

/** A press being dragged. */
type Gesture =
  | { kind: "move"; start: Pt; at: Pt; parts: Set<string>; notes: Set<string>; items: string[] }
  | { kind: "marquee"; start: Pt; at: Pt; add: boolean }
  | { kind: "arrow"; from: ArrowEnd; at: Pt; free: boolean }
  | { kind: "end"; arrow: string; which: "from" | "to"; at: Pt; free: boolean }
  | { kind: "bend"; key: string; start: Pt; at: Pt; from: Pt };

/** A note being typed. */
export interface Draft {
  id?: string;
  at: Pt;
  text: string;
  size: NoteSize;
  color: Swatch | null;
}

/** A drag shorter than this, in world units, is a click. */
const SLOP = 3;
/** What a design edge draws in when its style gives no color. */
const NEW = "#a371f7";

const ORIGIN: Pt = { x: 0, y: 0 };
const round = (p: Pt): Pt => ({ x: Math.round(p.x), y: Math.round(p.y) });
const centerOf = (b: Box): Pt => ({ x: b.x + b.w / 2, y: b.y + b.h / 2 });

export function edgePaint(style: EdgeStyle, e: SEdge | undefined): Paint {
  return {
    color: style.color ? swatchHex(style.color) : e?.status === "added" ? NEW : BLUE,
    dash: style.pattern === "dashed" ? DASHED : style.pattern === "dotted" ? DOTTED : undefined,
    width: style.weight === "thick" ? 3 : 1.6,
    route: style.route,
    bend: style.bend,
  };
}

export const defaultStyle = (): EdgeStyle => ({
  pattern: "solid",
  color: null,
  weight: "thin",
  route: "curved",
  bend: null,
  by: "user",
});

/** Editing a design view's canvas: what it draws, the selection, the tools, and the presses
 * and keys that change it. */
export function useCanvasEdit(p: {
  canvas: Canvas;
  commit(c: Canvas): void;
  view: string;
  base: Layout;
  edges: SEdge[];
  editing: boolean;
}) {
  const { canvas, view: key, editing } = p;
  const v = useMemo(() => viewOf(canvas, key), [canvas, key]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [tool, setTool] = useState<Tool>("select");
  const [gesture, setGesture] = useState<Gesture>();
  const [draft, setDraft] = useState<Draft>();
  // Cleared when a draft is saved or dropped, so a blur as it closes doesn't save it again.
  const drafting = useRef<Draft | undefined>(undefined);
  drafting.current = draft;
  const [renaming, setRenaming] = useState<string>();
  const [panning, setPanning] = useState(false);
  const live = useRef<Gesture | undefined>(undefined);
  live.current = gesture;
  const toWorldRef = useRef<ToWorld | undefined>(undefined);

  // A new view or design starts with nothing selected.
  // biome-ignore lint/correctness/useExhaustiveDependencies: resets on a new view
  useEffect(() => {
    setSelected(new Set());
    setDraft(undefined);
  }, [key]);
  useEffect(() => {
    if (!editing) {
      setSelected(new Set());
      setTool("select");
      setDraft(undefined);
    }
  }, [editing]);

  useEffect(() => {
    if (!editing) return;
    const typing = (e: globalThis.KeyboardEvent) =>
      e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
    const down = (e: globalThis.KeyboardEvent) => {
      if (e.key === " " && !typing(e)) setPanning(true);
    };
    const up = (e: globalThis.KeyboardEvent) => {
      if (e.key === " ") setPanning(false);
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
    };
  }, [editing]);

  const plain = useMemo(
    () => arrange(p.base, p.edges, v, canvas.colors, undefined, v.notes.map(noteBox)),
    [p.base, p.edges, v, canvas.colors],
  );
  const moving = gesture?.kind === "move" ? gesture : undefined;
  const dx = moving ? moving.at.x - moving.start.x : 0;
  const dy = moving ? moving.at.y - moving.start.y : 0;
  const notes = v.notes.map((n) =>
    moving?.notes.has(n.id) ? { ...n, at: { x: n.at.x + dx, y: n.at.y + dy } } : n,
  );
  const noteBoxes = new Map(notes.map((n) => [n.id, noteBox(n)]));
  const arranged: Arranged = moving?.parts.size
    ? arrange(p.base, p.edges, v, canvas.colors, { ids: moving.parts, dx, dy }, [
        ...noteBoxes.values(),
      ])
    : plain;
  // While parts move, frames keep to where they were, save those of moving groups.
  const frames = useMemo(() => {
    const boxes = groupBoxes(v, moving ? plain.boxes : arranged.boxes);
    if (!moving) return boxes;
    const carried = new Set(
      moving.items.flatMap((i) => {
        const r = parseRef(i);
        return r.kind === "group" ? groupsIn(v, r.id) : [];
      }),
    );
    return new Map(
      [...boxes].map(([id, b]) => [id, carried.has(id) ? { ...b, x: b.x + dx, y: b.y + dy } : b]),
    );
  }, [v, moving, plain.boxes, arranged.boxes, dx, dy]);

  const boxOf = (target: string): Box | undefined =>
    arranged.boxes.get(target) ?? noteBoxes.get(target) ?? frames.get(target);

  const endPoint = (end: ArrowEnd, other: ArrowEnd): Pt => {
    const box = end.target ? boxOf(end.target) : undefined;
    if (!box) return end.at ?? ORIGIN;
    const ob = other.target ? boxOf(other.target) : undefined;
    return boxExit(box, ob ? centerOf(ob) : (other.at ?? ORIGIN));
  };

  /** The part, note or innermost group under `pt`. */
  const targetAt = (pt: Pt, except = new Set<string>()): string | undefined => {
    const hit = (b: Box) => pt.x >= b.x && pt.x <= b.x + b.w && pt.y >= b.y && pt.y <= b.y + b.h;
    const part = [...arranged.boxes].reverse().find(([id, b]) => !except.has(id) && hit(b));
    if (part) return part[0];
    const note = [...noteBoxes].reverse().find(([id, b]) => !except.has(id) && hit(b));
    if (note) return note[0];
    return groupAt(frames, pt.x, pt.y, except);
  };

  const paints = useMemo(() => {
    const byKey = new Map(p.edges.map((e) => [`${e.from}>${e.to}`, e]));
    const out = new Map<string, Paint>();
    for (const [k, style] of Object.entries(v.edges)) {
      const bend =
        gesture?.kind === "bend" && gesture.key === k
          ? {
              x: gesture.from.x + gesture.at.x - gesture.start.x,
              y: gesture.from.y + gesture.at.y - gesture.start.y,
            }
          : style.bend;
      out.set(k, edgePaint({ ...style, bend }, byKey.get(k)));
    }
    return out;
  }, [v.edges, p.edges, gesture]);

  const change = (f: (v: View) => View) => p.commit(withView(canvas, key, f));

  /** Starts tracking a press from world point `start`, ending in `done`. */
  const track = (
    e: React.PointerEvent,
    toWorld: ToWorld,
    first: Gesture,
    done: (g: Gesture, moved: boolean, e: PointerEvent) => void,
  ) => {
    toWorldRef.current = toWorld;
    setGesture(first);
    const start = toWorld(e.clientX, e.clientY);
    let moved = false;
    const move = (ev: PointerEvent) => {
      const at = toWorld(ev.clientX, ev.clientY);
      if (!moved && Math.hypot(at.x - start.x, at.y - start.y) < SLOP) return;
      moved = true;
      setGesture(
        (g) => g && ({ ...g, at, ...("free" in g ? { free: ev.altKey } : {}) } as Gesture),
      );
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      const g = live.current;
      setGesture(undefined);
      if (g) done({ ...g, at: toWorld(ev.clientX, ev.clientY) } as Gesture, moved, ev);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const finishMove = (g: Extract<Gesture, { kind: "move" }>) => {
    const ddx = g.at.x - g.start.x;
    const ddy = g.at.y - g.start.y;
    change((view) => {
      let out = view;
      if (g.parts.size) {
        out = out.frozen ? out : freeze(out, plain.layout);
        const positions = { ...out.positions };
        for (const id of g.parts) {
          const b = plain.boxes.get(id);
          const from = positions[id] ?? b;
          if (from)
            positions[id] = round({ x: Math.max(0, from.x + ddx), y: Math.max(0, from.y + ddy) });
        }
        out = { ...out, positions };
      }
      if (g.notes.size)
        out = {
          ...out,
          notes: out.notes.map((n) =>
            g.notes.has(n.id) ? { ...n, at: round({ x: n.at.x + ddx, y: n.at.y + ddy }) } : n,
          ),
        };
      const items = g.items.flatMap((i) => {
        const r = parseRef(i);
        return r.kind === "part" || r.kind === "group" ? [r.id] : [];
      });
      if (items.length) {
        const carried = new Set(items.flatMap((i) => (isPart(i) ? [] : groupsIn(out, i))));
        const into = groupAt(groupBoxes(out, plain.boxes), g.at.x, g.at.y, carried);
        const leaving = items.filter((i) => parentOf(out, i) !== into);
        if (leaving.length) out = moveInto(out, leaving, into);
      }
      return out;
    });
  };

  /** The things a press on `ref` moves: the selection when it's in it, else itself. */
  const startMove = (ref: string, e: React.PointerEvent, toWorld: ToWorld) => {
    let sel = selected;
    if (e.shiftKey) {
      sel = new Set(selected);
      if (sel.has(ref)) sel.delete(ref);
      else sel.add(ref);
    } else if (!selected.has(ref)) sel = new Set([ref]);
    setSelected(sel);
    const items = [...sel];
    const parts = new Set(idsOf(sel, "part"));
    const notesMoving = new Set(idsOf(sel, "note"));
    for (const g of idsOf(sel, "group")) for (const part of partsIn(v, g)) parts.add(part);
    const start = toWorld(e.clientX, e.clientY);
    track(
      e,
      toWorld,
      { kind: "move", start, at: start, parts, notes: notesMoving, items },
      (g, moved) => {
        if (moved && g.kind === "move") finishMove(g);
      },
    );
  };

  const startArrow = (e: React.PointerEvent, toWorld: ToWorld, target?: string) => {
    const at = round(toWorld(e.clientX, e.clientY));
    const from: ArrowEnd = { target: e.altKey ? null : (target ?? targetAt(at) ?? null), at };
    track(e, toWorld, { kind: "arrow", from, at, free: e.altKey }, (g, moved) => {
      if (!moved || g.kind !== "arrow") return;
      const to: ArrowEnd = {
        target: g.free
          ? null
          : (targetAt(g.at, new Set(g.from.target ? [g.from.target] : [])) ?? null),
        at: round(g.at),
      };
      const id = freshId(v, "a");
      const arrow: CanvasArrow = {
        id,
        from: g.from,
        to,
        color: null,
        pattern: "solid",
        by: "user",
      };
      change((view) => ({ ...view, arrows: [...view.arrows, arrow] }));
      setSelected(new Set([arrowRef(id)]));
      setTool("select");
    });
  };

  const startText = (e: React.PointerEvent, toWorld: ToWorld) => {
    setDraft({ at: round(toWorld(e.clientX, e.clientY)), text: "", size: "m", color: null });
    setTool("select");
  };

  const onNodeDown = (id: string, e: React.PointerEvent, toWorld: ToWorld) => {
    if (tool === "arrow") return startArrow(e, toWorld, id);
    if (tool === "text") return startText(e, toWorld);
    startMove(id, e, toWorld);
  };

  const onBackgroundDown = (e: React.PointerEvent, toWorld: ToWorld) => {
    if (tool === "arrow") return startArrow(e, toWorld);
    if (tool === "text") return startText(e, toWorld);
    const start = toWorld(e.clientX, e.clientY);
    track(e, toWorld, { kind: "marquee", start, at: start, add: e.shiftKey }, (g, moved) => {
      if (g.kind !== "marquee") return;
      if (!moved) {
        if (!g.add) setSelected(new Set());
        return;
      }
      const r = {
        x: Math.min(g.start.x, g.at.x),
        y: Math.min(g.start.y, g.at.y),
        w: Math.abs(g.at.x - g.start.x),
        h: Math.abs(g.at.y - g.start.y),
      };
      const meets = (b: Box) =>
        b.x < r.x + r.w && r.x < b.x + b.w && b.y < r.y + r.h && r.y < b.y + b.h;
      const hits = [
        ...[...arranged.boxes].filter(([, b]) => meets(b)).map(([id]) => id),
        ...[...noteBoxes].filter(([, b]) => meets(b)).map(([id]) => noteRef(id)),
      ];
      setSelected(new Set([...(g.add ? selected : []), ...hits]));
    });
  };

  /** Presses on the canvas's own elements: group labels, notes, arrows and handles. */
  const press = {
    group: (id: string, e: React.PointerEvent) => {
      e.stopPropagation();
      const tw = toWorldRef.current;
      if (!tw || e.button !== 0) return;
      if (tool === "arrow") return startArrow(e, tw, id);
      startMove(groupRef(id), e, tw);
    },
    note: (id: string, e: React.PointerEvent) => {
      e.stopPropagation();
      const tw = toWorldRef.current;
      if (!tw || e.button !== 0) return;
      if (tool === "arrow") return startArrow(e, tw, id);
      startMove(noteRef(id), e, tw);
    },
    arrow: (id: string, e: React.PointerEvent) => {
      e.stopPropagation();
      if (e.button !== 0) return;
      setSelected(e.shiftKey ? new Set([...selected, arrowRef(id)]) : new Set([arrowRef(id)]));
    },
    end: (id: string, which: "from" | "to", e: React.PointerEvent) => {
      e.stopPropagation();
      const tw = toWorldRef.current;
      if (!tw || e.button !== 0) return;
      const at = tw(e.clientX, e.clientY);
      track(e, tw, { kind: "end", arrow: id, which, at, free: e.altKey }, (g, moved) => {
        if (!moved || g.kind !== "end") return;
        change((view) => ({
          ...view,
          arrows: view.arrows.map((a) => {
            if (a.id !== id) return a;
            const other = which === "from" ? a.to : a.from;
            const end: ArrowEnd = {
              target: g.free
                ? null
                : (targetAt(g.at, new Set(other.target ? [other.target] : [])) ?? null),
              at: round(g.at),
            };
            return { ...a, [which]: end };
          }),
        }));
      });
    },
    /** `from` is the bend that draws the edge where it is now. */
    bend: (edge: string, e: React.PointerEvent, from: Pt) => {
      e.stopPropagation();
      const tw = toWorldRef.current;
      if (!tw || e.button !== 0) return;
      const start = tw(e.clientX, e.clientY);
      track(e, tw, { kind: "bend", key: edge, start, at: start, from }, (g, moved) => {
        if (!moved || g.kind !== "bend") return;
        const bend = round({ x: from.x + g.at.x - g.start.x, y: from.y + g.at.y - g.start.y });
        change((view) => ({
          ...view,
          edges: {
            ...view.edges,
            [edge]: { ...(view.edges[edge] ?? defaultStyle()), bend, by: "user" },
          },
        }));
      });
    },
    noteText: (id: string) => {
      const n = v.notes.find((x) => x.id === id);
      if (n) setDraft({ id, at: n.at, text: n.text, size: n.size, color: n.color });
    },
  };

  const cancelDraft = () => {
    drafting.current = undefined;
    setDraft(undefined);
  };

  const commitDraft = (d: Draft) => {
    if (!drafting.current) return;
    cancelDraft();
    const text = d.text.trim();
    if (!d.id && !text) return;
    if (d.id && !text) {
      change((view) => ({ ...view, notes: view.notes.filter((n) => n.id !== d.id) }));
      return;
    }
    const id = d.id ?? freshId(v, "n");
    const note: CanvasNote = { id, text, at: d.at, size: d.size, color: d.color, by: "user" };
    change((view) => ({
      ...view,
      notes: d.id ? view.notes.map((n) => (n.id === id ? note : n)) : [...view.notes, note],
    }));
    setSelected(new Set([noteRef(id)]));
  };

  const groupSelection = () => {
    const items = [...idsOf(selected, "part"), ...idsOf(selected, "group")];
    if (!items.length) return;
    const id = freshId(v, "g");
    change((view) => group(view, items, id, "Group"));
    setSelected(new Set([groupRef(id)]));
    setRenaming(id);
  };

  const ungroupSelection = () => {
    const ids = idsOf(selected, "group");
    if (!ids.length) return;
    change((view) => ids.reduce(ungroup, view));
    setSelected(new Set());
  };

  const removeSelection = () => {
    const notesGone = new Set(idsOf(selected, "note"));
    const arrowsGone = new Set(idsOf(selected, "arrow"));
    const groupsGone = idsOf(selected, "group");
    const edgesGone = new Set(idsOf(selected, "edge"));
    if (!notesGone.size && !arrowsGone.size && !groupsGone.length && !edgesGone.size) return;
    change((view) => ({
      ...groupsGone.reduce(ungroup, view),
      notes: view.notes.filter((n) => !notesGone.has(n.id)),
      arrows: view.arrows.filter((a) => !arrowsGone.has(a.id)),
      edges: Object.fromEntries(Object.entries(view.edges).filter(([k]) => !edgesGone.has(k))),
    }));
    setSelected(new Set());
  };

  /** Keys while editing; true when one was taken. */
  const onKey = (e: KeyboardEvent): boolean => {
    if (!editing) return false;
    const mod = e.metaKey || e.ctrlKey;
    const k = e.key.toLowerCase();
    if (mod && k === "g") {
      if (e.shiftKey) ungroupSelection();
      else groupSelection();
    } else if (mod) return false;
    else if (k === "v") setTool("select");
    else if (k === "t") setTool("text");
    else if (k === "a") setTool("arrow");
    else if (e.key === "Backspace" || e.key === "Delete") removeSelection();
    else if (e.key === "Escape" && tool !== "select") setTool("select");
    else if (e.key === "Escape" && selected.size) setSelected(new Set());
    else return false;
    return true;
  };

  /** The selection's box in the world, for placing its toolbar. */
  const selectionBox = (): Box | undefined => {
    const boxes = [...selected].flatMap((r): Box[] => {
      const ref = parseRef(r);
      if (ref.kind === "part")
        return arranged.boxes.get(ref.id) ? [arranged.boxes.get(ref.id) as Box] : [];
      if (ref.kind === "group") return frames.get(ref.id) ? [frames.get(ref.id) as GroupBox] : [];
      if (ref.kind === "note") return noteBoxes.get(ref.id) ? [noteBoxes.get(ref.id) as Box] : [];
      if (ref.kind === "arrow") {
        const a = v.arrows.find((x) => x.id === ref.id);
        if (!a) return [];
        const s = endPoint(a.from, a.to);
        const t = endPoint(a.to, a.from);
        return [
          {
            x: Math.min(s.x, t.x),
            y: Math.min(s.y, t.y),
            w: Math.abs(t.x - s.x),
            h: Math.abs(t.y - s.y),
          },
        ];
      }
      const e = p.edges.find((x) => `${x.from}>${x.to}` === ref.id);
      const a = e && arranged.boxes.get(e.from);
      const b = e && arranged.boxes.get(e.to);
      if (!a || !b) return [];
      const ca = centerOf(a);
      const cb = centerOf(b);
      return [
        {
          x: Math.min(ca.x, cb.x),
          y: Math.min(ca.y, cb.y),
          w: Math.abs(cb.x - ca.x),
          h: Math.abs(cb.y - ca.y),
        },
      ];
    });
    if (!boxes.length) return undefined;
    const x = Math.min(...boxes.map((b) => b.x));
    const y = Math.min(...boxes.map((b) => b.y));
    return {
      x,
      y,
      w: Math.max(...boxes.map((b) => b.x + b.w)) - x,
      h: Math.max(...boxes.map((b) => b.y + b.h)) - y,
    };
  };

  const dropTarget = moving?.items.some((i) => ["part", "group"].includes(parseRef(i).kind))
    ? groupAt(
        groupBoxes(v, plain.boxes),
        moving.at.x,
        moving.at.y,
        new Set(
          moving.items.flatMap((i) => {
            const r = parseRef(i);
            return r.kind === "group" ? groupsIn(v, r.id) : [];
          }),
        ),
      )
    : undefined;

  return {
    viewKey: key,
    view: v,
    arranged,
    frames,
    notes,
    paints,
    selected,
    setSelected,
    tool,
    setTool,
    gesture,
    draft,
    setDraft,
    commitDraft,
    cancelDraft,
    renaming,
    setRenaming,
    panning,
    dropTarget,
    endPoint,
    press,
    onNodeDown,
    onBackgroundDown,
    onKey,
    change,
    groupSelection,
    ungroupSelection,
    removeSelection,
    selectionBox,
    selectEdge: (key: string, add: boolean) =>
      setSelected(add ? new Set([...selected, edgeRef(key)]) : new Set([edgeRef(key)])),
    resetLayout: () => change((view) => ({ ...view, positions: {}, frozen: false })),
    setToWorld: (tw: ToWorld) => {
      toWorldRef.current = tw;
    },
  };
}
