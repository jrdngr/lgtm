import { useEffect, useRef } from "react";
import type { CanvasShape } from "../bindings";
import { tint } from "../system/colors";
import { swatchHex } from "./palette";
import type { Label } from "./tools";
import type { useCanvasEdit } from "./useCanvasEdit";

type Ed = ReturnType<typeof useCanvasEdit>;

/** A shape without a color. */
const LINE = "#8b8d93";
const FILL = "#212226";

/** How shape `s` paints: its outline and inside. */
export function shapePaint(s: Pick<CanvasShape, "color" | "fill">) {
  const stroke = s.color ? swatchHex(s.color) : LINE;
  const fill = !s.fill ? "transparent" : s.color ? tint(stroke, 0.14) : FILL;
  return { stroke, fill };
}

/** The drawn shapes, under the arrows. */
export function Shapes({ ed, editing, flash }: { ed: Ed; editing: boolean; flash?: Set<string> }) {
  const one = ed.selected.size === 1 ? [...ed.selected][0] : undefined;
  return (
    <>
      {ed.shapes.map((s) => {
        const on = editing && ed.selected.has(s.id);
        const { stroke, fill } = shapePaint(s);
        const dashed = s.pattern !== "solid";
        return (
          <div
            key={s.id}
            className={`cv-shape cv-hit ${s.kind}${on ? " on" : ""}${flash?.has(s.id) ? " cv-flash" : ""}`}
            style={{ left: s.at.x, top: s.at.y, width: s.w, height: s.h }}
            onPointerDown={(e) => ed.press.shape(s.id, e)}
            onDoubleClick={() => ed.press.shapeText(s.id)}
          >
            {s.kind === "diamond" ? (
              <svg width={s.w} height={s.h} aria-hidden="true">
                <polygon
                  points={`${s.w / 2},1 ${s.w - 1},${s.h / 2} ${s.w / 2},${s.h - 1} 1,${s.h / 2}`}
                  fill={fill}
                  stroke={stroke}
                  strokeWidth="1.5"
                  strokeDasharray={dashed ? "7 5" : undefined}
                />
              </svg>
            ) : (
              <span
                className="cv-shape-body"
                style={{
                  borderColor: stroke,
                  borderStyle: dashed ? "dashed" : "solid",
                  background: fill,
                }}
              />
            )}
            {ed.label?.id === s.id ? (
              <ShapeLabel ed={ed} label={ed.label} />
            ) : (
              s.text && <span className="cv-shape-text">{s.text}</span>
            )}
            {on &&
              one === s.id &&
              [0, 1, 2, 3].map((c) => (
                <span
                  key={c}
                  className={`cv-handle c${c}`}
                  onPointerDown={(e) => ed.press.resize(s.id, c, e)}
                />
              ))}
          </div>
        );
      })}
    </>
  );
}

/** A shape's text being typed: Enter or leaving it saves, Escape drops it. */
function ShapeLabel({ ed, label }: { ed: Ed; label: Label }) {
  const ref = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    const t = setTimeout(() => {
      ref.current?.focus();
      ref.current?.select();
    });
    return () => clearTimeout(t);
  }, []);
  return (
    <textarea
      ref={ref}
      className="cv-shape-text cv-shape-input"
      rows={Math.max(1, label.text.split("\n").length)}
      value={label.text}
      aria-label="Text"
      onPointerDown={(e) => e.stopPropagation()}
      onChange={(e) => ed.setLabel({ ...label, text: e.target.value })}
      onBlur={() => ed.commitLabel(label)}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") ed.cancelLabel();
        else if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          ed.commitLabel(label);
        }
      }}
    />
  );
}

/** The tags, each at its element's top right corner. */
export function Tags({ ed, editing }: { ed: Ed; editing: boolean }) {
  const g = ed.gesture;
  return (
    <>
      {Object.entries(ed.view.tags).map(([digit, target]) => {
        if (g?.kind === "tag" && g.digit === digit) return null;
        const b = ed.tagBox(target);
        if (!b) return null;
        return (
          <span
            key={digit}
            className={`cv-tag${editing ? " cv-hit" : ""}`}
            style={{ left: b.x + b.w, top: b.y }}
            onPointerDown={editing ? (e) => ed.press.tag(digit, e) : undefined}
          >
            {digit}
          </span>
        );
      })}
      {g?.kind === "tag" && (
        <span className="cv-tag lifted" style={{ left: g.at.x, top: g.at.y }}>
          {g.digit}
        </span>
      )}
    </>
  );
}
