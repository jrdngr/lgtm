import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Marked } from "./FindPalette";
import { matchTargets, type Target } from "./traceTargets";

interface Props {
  label: string;
  value?: Target;
  /** Whether this end is the one being picked. */
  editing: boolean;
  targets: Target[];
  /** The other end, which this one can't be. */
  skip?: string;
  onEdit(): void;
  onPick(id: string): void;
}

/** The list's width. */
const DROP = 330;

export function TargetSwatch({ t }: { t: Target }) {
  const round = t.kind === "person" || t.kind === "external";
  return <span className={`sys-sw${round ? " round" : ""}`} style={{ background: t.color }} />;
}

/** One end of a trace: its node, or a search over every node a path can join. */
export function TraceField({ label, value, editing, targets, skip, onEdit, onPick }: Props) {
  const box = useRef<HTMLLabelElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const [q, setQ] = useState("");
  const [at, setAt] = useState(0);
  const [focused, setFocused] = useState(false);

  useEffect(() => {
    if (editing) input.current?.focus();
  }, [editing]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new value or leaving clears the query
  useEffect(() => setQ(""), [editing, value?.id]);

  const hits = focused ? matchTargets(targets, q, skip) : [];
  const pick = (t: Target | undefined) => t && onPick(t.id);

  const onKey = (e: KeyboardEvent) => {
    // An empty field lets Escape through to end the trace.
    if (e.key === "Escape" && !q) return;
    e.stopPropagation();
    if (e.key === "Escape") setQ("");
    else if (e.key === "ArrowDown") setAt((i) => Math.min(hits.length - 1, i + 1));
    else if (e.key === "ArrowUp") setAt((i) => Math.max(0, i - 1));
    else if (e.key === "Enter") pick(hits[at]);
    else return;
    e.preventDefault();
  };

  // The bar clips what overflows it, so the list hangs from the panel.
  const panel = box.current?.closest<HTMLElement>(".sys-panel");
  const list = () => {
    if (!panel || !box.current || !hits.length) return null;
    const p = panel.getBoundingClientRect();
    const r = box.current.getBoundingClientRect();
    const left = Math.max(8, Math.min(r.left - p.left, p.width - DROP - 8));
    return createPortal(
      <div className="sys-tdrop" style={{ top: r.bottom - p.top + 6, left, width: DROP }}>
        {hits.map((t, i) => (
          <button
            key={t.id}
            type="button"
            className={`sys-topt${i === at ? " on" : ""}`}
            onMouseDown={(e) => e.preventDefault()}
            onMouseEnter={() => setAt(i)}
            onClick={() => pick(t)}
          >
            <TargetSwatch t={t} />
            <span className="sys-topt-n">
              <Marked text={t.name} q={q.trim()} />
            </span>
            <span className="sys-topt-k">{t.where ? `${t.kind} · ${t.where}` : t.kind}</span>
          </button>
        ))}
      </div>,
      panel,
    );
  };

  return (
    // biome-ignore lint/a11y/noLabelWithoutControl: it holds the input, or the button that edits
    <label ref={box} className={`sys-tf${editing ? " on" : ""}`}>
      <span className="sys-tf-lab">{label}</span>
      {editing || !value ? (
        <input
          ref={input}
          value={q}
          placeholder={value?.name ?? "Search"}
          aria-label={label}
          spellCheck={false}
          onFocus={() => {
            setFocused(true);
            if (!editing) onEdit();
          }}
          onBlur={() => setFocused(false)}
          onChange={(e) => {
            setQ(e.target.value);
            setAt(0);
          }}
          onKeyDown={onKey}
        />
      ) : (
        <button type="button" className="sys-tf-val" onClick={onEdit}>
          <TargetSwatch t={value} />
          <span className="sys-cn">{value.name}</span>
        </button>
      )}
      {list()}
    </label>
  );
}
