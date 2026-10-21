import { useEffect, useRef } from "react";
import type { ArrowEnd, CanvasArrow, Swatch } from "../../bindings";
import { tint } from "../../system/colors";
import { arrowPath, type Pt, shapedPath } from "../../system/edgePaths";
import { NOTE_PX } from "./notes";
import { INK, swatchHex } from "./palette";
import { arrowRef, groupRef, noteRef, parseRef } from "./refs";
import type { Draft, useCanvasEdit } from "./useCanvasEdit";

type Ed = ReturnType<typeof useCanvasEdit>;

const GROUP = "#8b8d93";
const hexOf = (c: Swatch | null | undefined, fallback: string) => (c ? swatchHex(c) : fallback);
const headId = (hex: string) => `cv-head-${hex.slice(1)}`;

/** Group frames, and the frame a drag would drop into: drawn in the world under the edges. */
export function CanvasUnder({ ed, editing }: { ed: Ed; editing: boolean }) {
  const drop = ed.dropTarget && ed.frames.get(ed.dropTarget);
  return (
    <>
      {[...ed.frames]
        .sort(([, a], [, b]) => a.depth - b.depth)
        .map(([id, b]) => {
          const g = ed.view.groups.find((x) => x.id === id);
          const c = hexOf(g?.color, GROUP);
          return (
            <div
              key={id}
              className={`cv-frame${editing && ed.selected.has(groupRef(id)) ? " on" : ""}`}
              style={{
                left: b.x,
                top: b.y,
                width: b.w,
                height: b.h,
                borderColor: tint(c, 0.5),
                background: tint(c, 0.05),
              }}
            />
          );
        })}
      {drop && (
        <div
          className="cv-drop"
          style={{ left: drop.x - 4, top: drop.y - 4, width: drop.w + 8, height: drop.h + 8 }}
        />
      )}
    </>
  );
}

/** Group labels, notes, arrows and the editing handles: drawn in the world over the nodes. */
export function CanvasOver({
  ed,
  editing,
  flash,
}: {
  ed: Ed;
  editing: boolean;
  flash?: Set<string>;
}) {
  const g = ed.gesture;
  const arrows: CanvasArrow[] = ed.view.arrows.map((a) => {
    if (g?.kind !== "end" || g.arrow !== a.id) return a;
    const free: ArrowEnd = { target: null, at: g.at };
    return { ...a, [g.which]: free };
  });
  const colors = new Set([INK, ...arrows.map((a) => hexOf(a.color, INK))]);
  const sel = (r: string) => editing && ed.selected.has(r);
  const lit = (id: string) => (flash?.has(id) ? " cv-flash" : "");
  const edgeSel = editing ? [...ed.selected].map(parseRef).filter((r) => r.kind === "edge") : [];
  const bend = edgeSel.length === 1 ? bendHandle(ed, edgeSel[0].id) : undefined;
  return (
    <div className={`cv-over${editing ? " editing" : ""}`}>
      {[...ed.frames].map(([id, b]) => {
        const grp = ed.view.groups.find((x) => x.id === id);
        if (!grp) return null;
        const c = hexOf(grp.color, GROUP);
        return (
          <div
            key={id}
            className={`cv-pill cv-hit${sel(groupRef(id)) ? " on" : ""}${lit(id)}`}
            style={{ left: b.x + 14, top: b.y - 10, borderColor: tint(c, 0.5), color: c }}
            onPointerDown={(e) => ed.press.group(id, e)}
            onDoubleClick={() => ed.setRenaming(id)}
          >
            <svg
              width="11"
              height="11"
              viewBox="0 0 12 12"
              fill="none"
              stroke={c}
              strokeWidth="1.3"
              aria-hidden="true"
            >
              <rect x="1" y="1" width="6" height="6" rx="1.5" />
              <rect x="5" y="5" width="6" height="6" rx="1.5" fill="var(--sys-bg, #161719)" />
            </svg>
            {grp.label}
          </div>
        );
      })}
      <svg className="cv-arrows" aria-hidden="true">
        <defs>
          {[...colors].map((c) => (
            <marker
              key={c}
              id={headId(c)}
              viewBox="0 0 12 12"
              refX="10"
              refY="6"
              markerWidth="12"
              markerHeight="12"
              markerUnits="userSpaceOnUse"
              orient="auto"
            >
              <path d="M0 0L12 6L0 12z" fill={c} />
            </marker>
          ))}
        </defs>
        {arrows.map((a) => {
          const p = ed.endPoint(a.from, a.to);
          const q = ed.endPoint(a.to, a.from);
          const d = arrowPath(p, q);
          const c = hexOf(a.color, INK);
          return (
            <g key={a.id} className={lit(a.id).trim()}>
              {sel(arrowRef(a.id)) && <path d={d} className="cv-arrow-sel" />}
              <path
                d={d}
                className="cv-arrow"
                stroke={c}
                strokeDasharray={a.pattern === "dashed" ? "9 7" : undefined}
                markerEnd={`url(#${headId(c)})`}
              />
              <path
                d={d}
                className="cv-arrow-hit cv-hit"
                onPointerDown={(e) => ed.press.arrow(a.id, e)}
              />
            </g>
          );
        })}
        {g?.kind === "arrow" && (
          <path
            d={arrowPath(ed.endPoint(g.from, { target: null, at: g.at }), g.at)}
            className="cv-arrow"
            stroke={INK}
            markerEnd={`url(#${headId(INK)})`}
          />
        )}
      </svg>
      {arrows
        .filter((a) => sel(arrowRef(a.id)))
        .flatMap((a) =>
          (["from", "to"] as const).map((which) => {
            const end = a[which];
            const at = ed.endPoint(end, which === "from" ? a.to : a.from);
            return (
              <span
                key={`${a.id}-${which}`}
                className={`cv-end cv-hit${end.target ? " stuck" : ""}`}
                style={{ left: at.x, top: at.y }}
                onPointerDown={(e) => ed.press.end(a.id, which, e)}
              />
            );
          }),
        )}
      {ed.notes
        .filter((n) => n.id !== ed.draft?.id)
        .map((n) => (
          <div
            key={n.id}
            className={`cv-note cv-hit${sel(noteRef(n.id)) ? " on" : ""}${lit(n.id)}`}
            style={{
              left: n.at.x,
              top: n.at.y,
              fontSize: NOTE_PX[n.size],
              color: hexOf(n.color, INK),
            }}
            onPointerDown={(e) => ed.press.note(n.id, e)}
            onDoubleClick={() => ed.press.noteText(n.id)}
          >
            {n.text}
          </div>
        ))}
      {ed.draft && <DraftNote ed={ed} draft={ed.draft} />}
      {bend && (
        <span
          className="cv-bend cv-hit"
          style={{ left: bend.mid.x, top: bend.mid.y }}
          onPointerDown={(e) => ed.press.bend(edgeSel[0].id, e, bend.from)}
        />
      )}
      {g?.kind === "marquee" && (
        <div
          className="cv-marquee"
          style={{
            left: Math.min(g.start.x, g.at.x),
            top: Math.min(g.start.y, g.at.y),
            width: Math.abs(g.at.x - g.start.x),
            height: Math.abs(g.at.y - g.start.y),
          }}
        />
      )}
    </div>
  );
}

/** Where a selected edge's bend handle sits, and the bend that puts its middle there. */
function bendHandle(ed: Ed, key: string): { mid: Pt; from: Pt } | undefined {
  const [from, to] = key.split(">");
  const a = ed.arranged.boxes.get(from);
  const b = ed.arranged.boxes.get(to);
  if (!a || !b) return undefined;
  const paint = ed.paints.get(key);
  const { mid } = shapedPath(a, b, paint ?? { route: "curved" });
  const half = { x: (a.x + a.w / 2 + b.x + b.w / 2) / 2, y: (a.y + a.h / 2 + b.y + b.h / 2) / 2 };
  return { mid, from: paint?.bend ?? { x: mid.x - half.x, y: mid.y - half.y } };
}

/** A note being typed: Enter or leaving it saves, Escape drops it. */
function DraftNote({ ed, draft }: { ed: Ed; draft: Draft }) {
  const ref = useRef<HTMLTextAreaElement>(null);
  // Focused after the press that opened it has settled focus on the canvas.
  useEffect(() => {
    const t = setTimeout(() => ref.current?.focus());
    return () => clearTimeout(t);
  }, []);
  const lines = draft.text.split("\n");
  return (
    <textarea
      ref={ref}
      className="cv-draft"
      rows={lines.length}
      cols={Math.max(8, ...lines.map((l) => l.length + 1))}
      value={draft.text}
      style={{
        left: draft.at.x,
        top: draft.at.y,
        fontSize: NOTE_PX[draft.size],
        color: hexOf(draft.color, INK),
      }}
      onPointerDown={(e) => e.stopPropagation()}
      onChange={(e) => ed.setDraft({ ...draft, text: e.target.value })}
      onBlur={() => ed.commitDraft(draft)}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") ed.cancelDraft();
        else if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          ed.commitDraft(draft);
        }
      }}
    />
  );
}
