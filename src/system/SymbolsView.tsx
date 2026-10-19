import { type ReactNode, useEffect, useMemo, useState } from "react";
import type { SymbolsDto, SymNodeDto, SystemDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import type { FlatFile } from "../review/model";
import { NEUTRAL, tint } from "./colors";
import { Fit, Minus, Plus } from "./glyphs";
import { curve } from "./layout";
import { usePanZoom } from "./panZoom";
import { SymbolDetail } from "./SymbolDetail";
import { KIND_LABEL, type SymBox, symbolsLayout } from "./symbolsLayout";

export type SymStatus = "added" | "changed";

interface Props {
  workspace: WorkspaceName;
  map: SystemDto;
  colors: Map<string, string>;
  component: string;
  /** The review's files, with the Review lens on. */
  files?: FlatFile[];
  /** Fades what the review doesn't change or reach. */
  changedOnly: boolean;
  /** A tour step's file or symbol. */
  tour?: { file?: string; symbol?: { path: string; name: string } };
  /** Replaces the detail, like a tour's steps. */
  aside?: ReactNode;
  /** The component's own detail, with no symbol selected. */
  fallback: ReactNode;
  /** Drawn over the canvas. */
  children?: ReactNode;
  selected?: number;
  onSelect(id: number | undefined): void;
  onShow(id: string): void;
  onFile(path: string, line?: number): void;
  onAsk(text: string): void;
}

/** The component's symbols, by file, with the symbols elsewhere that call them and that they
 * call. */
export function SymbolsView(p: Props) {
  const [got, setGot] = useState<{ component: string; dto?: SymbolsDto; error?: string }>();
  // biome-ignore lint/correctness/useExhaustiveDependencies: a reloaded map asks again
  useEffect(() => {
    let live = true;
    const component = p.component;
    unwrap(commands.systemSymbols(p.workspace, component)).then(
      (dto) => live && setGot({ component, dto }),
      (e) => live && setGot({ component, error: errorText(e) }),
    );
    return () => {
      live = false;
    };
  }, [p.workspace, p.component, p.map]);
  const mine = got?.component === p.component ? got : undefined;
  if (mine?.dto?.symbols.length) return <Loaded {...p} dto={mine.dto} />;
  return (
    <>
      <div className="sys-canvas">
        <div className={`placeholder${mine?.error ? " error" : ""}`}>
          {mine?.error ?? (mine?.dto ? "No symbols" : "Finding symbols…")}
        </div>
        {p.children}
      </div>
      {p.aside ?? p.fallback}
    </>
  );
}

/** Whether the review adds the symbol's file or changes rows in it. */
export function symbolStatus(files: FlatFile[], s: SymNodeDto): SymStatus | undefined {
  const f = files.find((x) => x.dto.path === s.path);
  if (!f) return undefined;
  if (f.dto.status === "added") return "added";
  const hit = f.rows.some(
    (r) => r.kind === "added" && r.newLine !== null && r.newLine >= s.line && r.newLine <= s.end,
  );
  return hit ? "changed" : undefined;
}

/** `id` and the symbols its links join it to. */
function around(dto: SymbolsDto, ids: Set<number>): Set<number> {
  const out = new Set(ids);
  for (const e of dto.edges) {
    if (ids.has(e.from)) out.add(e.to);
    if (ids.has(e.to)) out.add(e.from);
  }
  return out;
}

const EDGE = "#3c3f45";
const AMBER = "#e3b341";
const GREEN = "#3fb950";

function Loaded({ dto, ...p }: Props & { dto: SymbolsDto }) {
  const placed = useMemo(() => symbolsLayout(dto), [dto]);
  const pz = usePanZoom(placed.width, placed.height);
  const { selected, onSelect: setSelected } = p;
  const color = p.colors.get(p.map.components.find((k) => k.id === p.component)?.container ?? "");
  const status = useMemo(() => {
    const out = new Map<number, SymStatus>();
    if (!p.files) return out;
    for (const s of dto.symbols) {
      const st = symbolStatus(p.files, s);
      if (st) out.set(s.id, st);
    }
    return out;
  }, [dto, p.files]);

  const tourSym = p.tour?.symbol;
  const tourId = tourSym
    ? dto.symbols.find((s) => s.path === tourSym.path && s.name === tourSym.name)?.id
    : undefined;
  const focus = tourId ?? selected;
  const lit =
    focus !== undefined
      ? around(dto, new Set([focus]))
      : p.tour?.file
        ? new Set(dto.symbols.filter((s) => s.path === p.tour?.file).map((s) => s.id))
        : p.changedOnly && status.size
          ? around(dto, new Set(status.keys()))
          : undefined;
  const box = new Map(placed.syms.map((b) => [b.sym.id, b]));
  const hot = p.files ? AMBER : (color ?? NEUTRAL);
  const sel = focus !== undefined ? box.get(focus)?.sym : undefined;

  const symClass = (b: SymBox) =>
    [
      "sys-sym",
      b.side !== "in" && "out",
      status.get(b.sym.id),
      b.sym.id === focus && (tourId !== undefined ? "tour" : "sel"),
      lit && !lit.has(b.sym.id) && "dim",
    ]
      .filter(Boolean)
      .join(" ");

  return (
    <>
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: Escape deselects from the panel */}
      <div
        className="sys-canvas"
        ref={pz.ref}
        onPointerDown={pz.onPointerDown}
        onClick={() => {
          if (!pz.dragged()) setSelected(undefined);
        }}
      >
        <div
          className="sys-world"
          style={{
            width: placed.width,
            height: placed.height,
            transform: `translate(${pz.view.x}px, ${pz.view.y}px) scale(${pz.view.k})`,
          }}
        >
          {placed.callers && (
            <span className="sys-cl" style={{ left: placed.callers.x, top: placed.callers.y }}>
              Callers
            </span>
          )}
          {placed.calls && (
            <span className="sys-cl" style={{ left: placed.calls.x, top: placed.calls.y }}>
              Calls
            </span>
          )}
          <div
            className="sys-boundary"
            style={{
              left: placed.frame.x,
              top: placed.frame.y,
              width: placed.frame.w,
              height: placed.frame.h,
              border: `1.5px solid ${tint(color ?? NEUTRAL, 0.45)}`,
              background: tint(color ?? NEUTRAL, 0.03),
            }}
          >
            <span style={{ color: tint(color ?? NEUTRAL, 0.85) }}>
              <span className="sys-sw" style={{ background: color ?? NEUTRAL }} />
              {p.map.components.find((k) => k.id === p.component)?.name ?? p.component}
            </span>
          </div>
          {placed.files.map((f) => {
            const slash = f.path.lastIndexOf("/");
            return (
              <div
                key={f.path}
                className={`sys-file-box${p.tour?.file === f.path ? " tour" : ""}`}
                style={{ left: f.x, top: f.y, width: f.w, height: f.h }}
              >
                <span>
                  {f.path.slice(0, slash + 1)}
                  <b>{f.path.slice(slash + 1)}</b>
                </span>
              </div>
            );
          })}
          <svg className="sys-edges" width={placed.width} height={placed.height} aria-hidden="true">
            <defs>
              {Object.entries({ base: EDGE, hot, added: GREEN }).map(([k, fill]) => (
                <marker
                  key={k}
                  id={`sym-arrow-${k}`}
                  viewBox="0 0 8 8"
                  refX="7"
                  refY="4"
                  markerWidth="8"
                  markerHeight="8"
                  markerUnits="userSpaceOnUse"
                  orient="auto"
                >
                  <path d="M0 0L8 4L0 8z" fill={fill} />
                </marker>
              ))}
            </defs>
            {dto.edges.map((e) => {
              const a = box.get(e.from);
              const b = box.get(e.to);
              if (!a || !b) return null;
              const on = focus !== undefined && (e.from === focus || e.to === focus);
              const added = status.get(e.to) === "added";
              const kind = added ? "added" : on ? "hot" : "base";
              const stroke = added ? GREEN : on ? hot : EDGE;
              const faint = lit && !(lit.has(e.from) && lit.has(e.to));
              return (
                <path
                  key={`${e.from}>${e.to}`}
                  className={`sys-edge${faint ? " faint" : ""}`}
                  d={curve(a, b).d}
                  fill="none"
                  style={{ stroke }}
                  strokeWidth={on || added ? 2 : 1.5}
                  markerEnd={`url(#sym-arrow-${kind})`}
                />
              );
            })}
          </svg>
          {placed.syms.map((b) => (
            <button
              key={b.sym.id}
              type="button"
              className={symClass(b)}
              style={{ left: b.x, top: b.y, width: b.w, height: b.h }}
              title={b.sym.signature}
              onClick={(ev) => {
                ev.stopPropagation();
                if (!pz.dragged()) setSelected(b.sym.id);
              }}
              onDoubleClick={() => p.onFile(b.sym.path, b.sym.line)}
            >
              <span className={`sys-k ${b.sym.kind}`}>{KIND_LABEL[b.sym.kind]}</span>
              <span className="sys-sym-nm">{b.sym.name}</span>
            </button>
          ))}
        </div>
        {p.children}
        <div className="sys-zoom">
          <button type="button" aria-label="Zoom In" onClick={pz.zoomIn}>
            <Plus />
          </button>
          <button type="button" aria-label="Zoom Out" onClick={pz.zoomOut}>
            <Minus />
          </button>
          <button type="button" aria-label="Fit" onClick={pz.fit}>
            <Fit />
          </button>
        </div>
      </div>
      {p.aside ??
        (sel ? (
          <SymbolDetail
            map={p.map}
            colors={p.colors}
            dto={dto}
            sym={sel}
            status={status.get(sel.id)}
            statusOf={(id) => status.get(id)}
            onSelect={setSelected}
            onShow={p.onShow}
            onFile={p.onFile}
            onAsk={p.onAsk}
          />
        ) : (
          p.fallback
        ))}
    </>
  );
}
