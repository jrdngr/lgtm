import { useEffect, useRef, useState } from "react";
import type { DesignItemDto, DesignMapDto, DesignState } from "../bindings";
import { revisionLabel, setSource, showCode, showDesign, useSource } from "./designs";
import "./design.css";
import { Lifted } from "../ui/Lifted";

const STATES: Record<DesignState, string> = { fresh: "Fresh", stale: "Stale", missing: "Missing" };

/** How many of a design's parts the code has otherwise. */
export const differsCount = (m: DesignMapDto | null) =>
  m
    ? [...m.containers, ...m.components, ...m.externals].filter((n) => n.built === "differs").length
    : 0;

interface Props {
  items: DesignItemDto[];
  onNew(): void;
}

/** Code or Design, the design shown with its picker, and whether it lies over the code. */
export function SourceSwitch({ items, onNew }: Props) {
  const source = useSource();
  const [open, setOpen] = useState(false);
  const shown = items.find((i) => i.design.id === source.design);
  return (
    <>
      <div className="dsn-source">
        <button
          type="button"
          className={shown ? undefined : "on"}
          aria-pressed={!shown}
          onClick={showCode}
        >
          Code
        </button>
        <button
          type="button"
          className={shown ? "on" : undefined}
          aria-pressed={!!shown}
          onClick={() => {
            if (shown) return;
            if (items[0]) showDesign(items[0].design.id);
            else onNew();
          }}
        >
          Design
        </button>
      </div>
      {shown && (
        <>
          <button
            type="button"
            className={`dsn-pick${open ? " open" : ""}`}
            aria-expanded={open}
            data-designs
            onClick={() => setOpen(!open)}
          >
            {shown.design.name} ▾
          </button>
          <button
            type="button"
            className={`dsn-over${source.overCode ? " on" : ""}`}
            aria-pressed={source.overCode}
            onClick={() => setSource({ overCode: !source.overCode })}
          >
            <span className="dsn-trk" />
            Over code
          </button>
        </>
      )}
      {open && shown && (
        <DesignPop
          items={items}
          shown={shown.design.id}
          onPick={(id) => {
            showDesign(id);
            setOpen(false);
          }}
          onNew={() => {
            setOpen(false);
            onNew();
          }}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  );
}

function DesignPop(p: {
  items: DesignItemDto[];
  shown: string;
  onPick(id: string): void;
  onNew(): void;
  onClose(): void;
}) {
  const root = useRef<HTMLDivElement>(null);
  const { onClose } = p;
  useEffect(() => {
    const away = (e: PointerEvent) => {
      const at = e.target as Element;
      if (!root.current?.contains(at) && !at.closest?.("[data-designs]")) onClose();
    };
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onClose]);
  return (
    <Lifted
      className="dsn-pop"
      role="menu"
      aria-label="Designs"
      ref={root}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
    >
      {p.items.map(({ design, state, fileCount, resolved }) => {
        const differs = differsCount(design.map);
        return (
          <button
            key={design.id}
            type="button"
            className={`dsn-row${design.id === p.shown ? " on" : ""}`}
            onClick={() => p.onPick(design.id)}
          >
            <span className="ck">{design.id === p.shown ? "✓" : ""}</span>
            <span className="nm">
              <b>{design.name}</b>
              <span>
                {revisionLabel({ design, state, fileCount, resolved })} · {fileCount}{" "}
                {fileCount === 1 ? "file" : "files"}
              </span>
            </span>
            <span className={`dsn-chip-state ${state}`}>{STATES[state]}</span>
            <span className="dsn-drift">{differs ? `${differs} differs` : ""}</span>
          </button>
        );
      })}
      <div className="dsn-sep" />
      <button type="button" className="dsn-new" onClick={p.onNew}>
        + New Design…
      </button>
    </Lifted>
  );
}
