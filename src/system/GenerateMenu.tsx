import { useEffect, useRef, useState } from "react";
import { PipeButton } from "../terminal/PipeButton";
import { useSending } from "../terminal/sending";
import { Spinner } from "../ui/Busy";
import type { GenItem, GenKey, GenState } from "./generate";
import { Chevron } from "./glyphs";
import "./gen.css";

const STATES: Record<GenState, string> = { fresh: "Fresh", stale: "Stale", missing: "Missing" };

/** Opens Generate; a dot when something is stale or missing, a spinner while a prompt is
 * being put together or the agent works on what it sent. Sends from its pop pass the same
 * `sendKey`. */
export function GenerateButton(p: {
  items: GenItem[];
  open: boolean;
  busy?: boolean;
  sendKey: string;
  onToggle(): void;
  className?: string;
}) {
  const busy = useSending(p.sendKey) || p.busy;
  return (
    <button
      type="button"
      className={`pipe bg sys-gen${p.className ? ` ${p.className}` : ""}${p.open ? " open" : ""}`}
      aria-expanded={p.open}
      aria-busy={busy}
      disabled={p.busy}
      data-generate
      onClick={p.onToggle}
    >
      Generate
      {!busy && p.items.some((i) => i.state !== "fresh") && <span className="sys-gen-dot" />}
      <span className="sys-down">{busy ? <Spinner /> : <Chevron size={10} />}</span>
    </button>
  );
}

interface PopProps {
  items: GenItem[];
  sendKey: string;
  className?: string;
  onGenerate(keys: GenKey[]): void;
  onClose(): void;
}

/** What to generate, stale and missing ones checked, and the button that sends them. */
export function GeneratePop({ items, sendKey, className, onGenerate, onClose }: PopProps) {
  const root = useRef<HTMLDivElement>(null);
  const [picked, setPicked] = useState(
    () => new Set(items.filter((i) => i.state !== "fresh").map((i) => i.key)),
  );
  useEffect(() => {
    const away = (e: PointerEvent) => {
      const at = e.target as Element;
      if (!root.current?.contains(at) && !at.closest?.("[data-generate]")) onClose();
    };
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onClose]);
  const keys = items.filter((i) => picked.has(i.key)).map((i) => i.key);
  const toggle = (key: GenKey) => {
    const next = new Set(picked);
    if (!next.delete(key)) next.add(key);
    setPicked(next);
  };
  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: keeps clicks off what's under it
    <div
      className={`sys-genpop${className ? ` ${className}` : ""}`}
      role="menu"
      aria-label="Generate"
      ref={root}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
    >
      {items.map((i) => (
        <label key={i.key} className="sys-genrow">
          <input type="checkbox" checked={picked.has(i.key)} onChange={() => toggle(i.key)} />
          <span className="sys-genname">{i.label}</span>
          <span className={`sys-genchip ${i.state}`}>{STATES[i.state]}</span>
        </label>
      ))}
      <div className="sys-gengo">
        <PipeButton
          background
          sendKey={sendKey}
          className="sm"
          disabled={!keys.length}
          onClick={() => onGenerate(keys)}
        >
          Generate {keys.length}
        </PipeButton>
      </div>
    </div>
  );
}
