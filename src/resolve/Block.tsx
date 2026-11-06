import { useState } from "react";
import type { ConflictView, Label, SideView } from "../bindings";
import { paint, type Segment } from "./paint";

/** A label: change id with its unique prefix, commit id, bookmarks, description. */
export function SideLabel({ label }: { label: Label }) {
  return (
    <>
      {label.changeId && (
        <span className="lr-id">
          <span className="lr-unique">{label.changeId.slice(0, label.unique)}</span>
          {label.changeId.slice(label.unique)} <span>{label.commitId}</span>
        </span>
      )}
      {label.bookmarks.map((b) => (
        <span key={b} className="lr-books">
          {b}
        </span>
      ))}
      <span className="lr-desc">{label.description}</span>
    </>
  );
}

interface LinesProps {
  side: SideView;
  /** The file line the first one is on, for kept sides. */
  first?: number;
  hl(text: string): Segment[][] | undefined;
  gone?: boolean;
  onDoubleClick?(): void;
}

/** A side's lines, numbered when kept, with the words it changed marked. */
function Lines({ side, first, hl, gone, onDoubleClick }: LinesProps) {
  const lines = side.text.split("\n");
  if (lines.length > 1 && lines[lines.length - 1] === "") lines.pop();
  const segs = hl(side.text);
  return (
    <div className={`rz-lines${gone ? " gone" : ""}`} onDoubleClick={onDoubleClick}>
      {lines.map((text, n) => {
        const tints = side.tints
          .filter((t) => t.line === n)
          .map((t) => [t.from, t.to] as [number, number]);
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: lines are by place
          <div key={n} className="rz-line">
            <span className="ln">{first === undefined ? "" : first + n}</span>
            <span className="code">{paint(text, segs?.[n] ?? [], tints)}</span>
          </div>
        );
      })}
    </div>
  );
}

interface EditProps {
  text: string;
  done(text: string): void;
}

/** A kept side's text being edited. Esc, ⌘Enter or leaving it ends the edit. */
function Edit({ text, done }: EditProps) {
  const [value, setValue] = useState(text);
  const rows = Math.max(1, value.split("\n").length - (value.endsWith("\n") ? 1 : 0));
  return (
    <textarea
      className="rz-edit"
      // biome-ignore lint/a11y/noAutofocus: the edit starts where it was asked for
      autoFocus
      spellCheck={false}
      rows={rows}
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onBlur={() => done(value)}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape" || (e.key === "Enter" && (e.metaKey || e.ctrlKey))) {
          e.preventDefault();
          done(value);
        }
      }}
    />
  );
}

export interface BlockProps {
  conflict: ConflictView;
  total: number;
  /** Whether keys act on it, and the kept side they act on. */
  current: boolean;
  pick: number | undefined;
  editing: number | undefined;
  removing: boolean;
  hl(text: string): Segment[][] | undefined;
  select(pick?: number): void;
  toggle(key: number): void;
  place(from: number, to: number): void;
  flip(pick: number): void;
  edit(pick: number | undefined): void;
  setText(pick: number, text: string): void;
}

/** One conflict: the sides it keeps in order, then the rest. While it keeps nothing, the
 * sides show their lines; once it keeps something, the rest are only labels. */
export function Block(p: BlockProps) {
  const { conflict: c } = p;
  const [dragging, setDragging] = useState<number>();
  const cls = ["rz-block", c.resolved ? "done" : "open", p.current && "current"];
  let line = c.line;
  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: the panel's keys move between conflicts
    <div className={cls.filter(Boolean).join(" ")} onClick={() => p.select()}>
      <div className="rz-head">
        <span>
          Conflict {c.index + 1} of {p.total}
        </span>
        <span className={`pill ${c.resolved ? "green" : "amber"}`}>
          {c.resolved ? "resolved" : "open"}
        </span>
      </div>
      {c.picks.map((s, i) => {
        const first = line;
        line += s.text.split("\n").length - (s.text.endsWith("\n") ? 1 : 0);
        return (
          <div
            key={s.key}
            className={`rz-kept${p.pick === i ? " selected" : ""}`}
            onDragOver={(e) => dragging !== undefined && e.preventDefault()}
            onDrop={() => {
              if (dragging !== undefined && dragging !== i) p.place(dragging, i);
              setDragging(undefined);
            }}
          >
            {/* biome-ignore lint/a11y/useKeyWithClickEvents: J and K move it; numbers drop it */}
            <div
              className="rz-side picked"
              onClick={(e) => {
                e.stopPropagation();
                p.select(i);
              }}
            >
              <span
                className="rz-grip"
                draggable
                onDragStart={() => setDragging(i)}
                onDragEnd={() => setDragging(undefined)}
              >
                ⠿
              </span>
              <Key k={s.key} on onClick={() => p.toggle(s.key)} />
              <SideLabel label={s.label} />
              <span className="spacer" />
              {s.edited && (
                <span className="mode-toggle">
                  {(["Edited", "Original"] as const).map((name) => (
                    <button
                      key={name}
                      type="button"
                      className="icon-button"
                      aria-pressed={(name === "Original") === s.original}
                      onClick={(e) => {
                        e.stopPropagation();
                        if ((name === "Original") !== s.original) p.flip(i);
                      }}
                    >
                      {name}
                    </button>
                  ))}
                </span>
              )}
            </div>
            {p.editing === i ? (
              <Edit
                text={s.text}
                done={(text) => {
                  p.edit(undefined);
                  if (text !== s.text) p.setText(i, text);
                }}
              />
            ) : (
              <Lines
                side={s}
                first={first}
                hl={p.hl}
                gone={p.removing}
                onDoubleClick={() => p.edit(i)}
              />
            )}
          </div>
        );
      })}
      {c.rest.map((s) => {
        const open = c.picks.length === 0 && s.key !== 0;
        return (
          <div key={s.key}>
            {/* biome-ignore lint/a11y/useKeyWithClickEvents: the side's number keeps it */}
            <div
              className={`rz-side${open ? "" : " rejected"}`}
              onClick={(e) => {
                e.stopPropagation();
                p.select();
                p.toggle(s.key);
              }}
            >
              {!open && <span className="rz-chev">▸</span>}
              <Key k={s.key} />
              <SideLabel label={s.label} />
            </div>
            {open && <Lines side={s} hl={p.hl} gone={p.removing} />}
          </div>
        );
      })}
    </div>
  );
}

function Key({ k, on, onClick }: { k: number; on?: boolean; onClick?(): void }) {
  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: the number key does the same
    <span
      className={`rz-key${on ? " on" : ""}`}
      onClick={
        onClick &&
        ((e) => {
          e.stopPropagation();
          onClick();
        })
      }
    >
      {k}
    </span>
  );
}
