import { type ReactNode, useEffect, useMemo, useState } from "react";
import type { SymbolsDto, SystemDto } from "../bindings";
import { NEUTRAL, tint } from "./colors";
import { Fit, Minus, Plus } from "./glyphs";
import type { Box } from "./layout";
import { usePanZoom } from "./panZoom";
import type { SymStatus } from "./SymbolsView";
import { KIND_LABEL, type SymLayout, symKey } from "./symbolsLayout";
import { focusLayout, hairlines, type Link, linksOf, wirePath } from "./symbolsLinks";

interface Props {
  dto: SymbolsDto;
  placed: SymLayout;
  map: SystemDto;
  colors: Map<string, string>;
  component: string;
  status: Map<number, SymStatus>;
  /** Fades what the review doesn't change or reach. */
  changedOnly: boolean;
  /** A tour step's file. */
  tourFile?: string;
  /** The node the canvas collapses around, and whether a tour picked it. */
  focus?: string;
  touring: boolean;
  onSelect(key: string | undefined): void;
  onFile(path: string, line?: number): void;
  children?: ReactNode;
}

const BLUE = "#5b9bd5";
const AMBER = "#e3b341";
const GREEN = "#3fb950";
/** How long a collapse or its undoing animates. */
const ANIMATE_MS = 550;

const idOf = (key: string) => (key.startsWith("s") ? Number(key.slice(1)) : undefined);

/** At rest, the component's files and symbols with a faint line per pair of files that call
 * each other; hovering a node draws its links. Selecting one collapses the canvas into it,
 * its callers and its calls, with every link drawn. */
export function SymbolsCanvas(p: Props) {
  const { dto, placed, status } = p;
  const [hover, setHover] = useState<string>();
  const focus = useMemo(
    () => (p.focus ? focusLayout(dto, placed, p.focus) : undefined),
    [dto, placed, p.focus],
  );
  const pz = usePanZoom(focus?.width ?? placed.width, focus?.height ?? placed.height);
  // Animates from the first render with a new focus, since the fit lands in that render.
  const [settled, setSettled] = useState(p.focus);
  const anim = settled !== p.focus;
  useEffect(() => {
    setHover(undefined);
    const t = setTimeout(() => setSettled(p.focus), ANIMATE_MS);
    return () => clearTimeout(t);
  }, [p.focus]);

  const color = p.colors.get(p.map.components.find((k) => k.id === p.component)?.container ?? "");
  const rest = useMemo(() => {
    const out = new Map<string, Box>();
    for (const c of placed.chips) out.set(c.key, c);
    for (const s of placed.syms) out.set(s.key, s);
    return out;
  }, [placed]);
  const lines = useMemo(() => hairlines(dto, placed), [dto, placed]);
  const boxOf = (key: string) => focus?.pos.get(key) ?? rest.get(key);

  const restLinks = hover && !focus ? linksOf(dto, placed, hover, true) : [];
  const links: Link[] = focus ? focus.links : restLinks;
  const lit = (() => {
    if (focus) return undefined;
    if (hover) return new Set([hover, ...restLinks.flat()]);
    if (p.tourFile) {
      return new Set(placed.syms.filter((s) => s.sym.path === p.tourFile).map((s) => s.key));
    }
    if (p.changedOnly && status.size) {
      const keys = new Set([...status.keys()].map(symKey));
      for (const k of [...keys])
        for (const [a, b] of linksOf(dto, placed, k, true)) {
          keys.add(a);
          keys.add(b);
        }
      return keys;
    }
    return undefined;
  })();

  const stroke = ([a, b]: Link) => {
    const sa = status.get(idOf(a) ?? -1);
    const sb = status.get(idOf(b) ?? -1);
    return sb === "added" ? GREEN : sa || sb ? AMBER : BLUE;
  };
  const markers = { blue: BLUE, amber: AMBER, green: GREEN };
  const marker = (c: string) => (c === GREEN ? "green" : c === AMBER ? "amber" : "blue");

  const nodeClass = (key: string, extra: (string | false | undefined)[]) =>
    [
      "sys-sym",
      ...extra,
      focus && !focus.pos.has(key) && "gone",
      key === p.focus && (p.touring ? "tour" : "sel"),
      lit && !lit.has(key) && "dim",
    ]
      .filter(Boolean)
      .join(" ");
  const place = (b: Box | undefined) =>
    b && { transform: `translate(${b.x}px, ${b.y}px)`, width: b.w, height: b.h };
  const nodeEvents = (key: string) => ({
    onPointerEnter: () => setHover(key),
    onPointerLeave: () => setHover((h) => (h === key ? undefined : h)),
    onClick: (ev: React.MouseEvent) => {
      ev.stopPropagation();
      if (!pz.dragged()) p.onSelect(key);
    },
  });
  const compName = (id: string) => p.map.components.find((k) => k.id === id)?.name ?? id;
  const compColor = (id: string) =>
    p.colors.get(p.map.components.find((k) => k.id === id)?.container ?? "") ?? NEUTRAL;

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: Escape deselects from the panel
    <div
      className={`sys-canvas${focus ? " sys-focus" : ""}`}
      ref={pz.ref}
      onPointerDown={pz.onPointerDown}
      onClick={() => {
        if (!pz.dragged()) p.onSelect(undefined);
      }}
    >
      <div
        className={`sys-world${anim ? " anim" : ""}`}
        style={{
          width: placed.width,
          height: placed.height,
          transform: `translate(${pz.view.x}px, ${pz.view.y}px) scale(${pz.view.k})`,
        }}
      >
        <div className="sys-sctx">
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
              {compName(p.component)}
            </span>
          </div>
          {placed.files.map((f) => {
            const slash = f.path.lastIndexOf("/");
            return (
              <div
                key={f.path}
                className={`sys-file-box${p.tourFile === f.path ? " tour" : ""}`}
                style={{ left: f.x, top: f.y, width: f.w, height: f.h }}
              >
                <span>
                  {f.path.slice(0, slash + 1)}
                  <b>{f.path.slice(slash + 1)}</b>
                </span>
              </div>
            );
          })}
        </div>
        <svg
          className="sys-edges"
          width={Math.max(placed.width, focus?.width ?? 0)}
          height={Math.max(placed.height, focus?.height ?? 0)}
          aria-hidden="true"
        >
          <defs>
            {Object.entries(markers).map(([k, fill]) => (
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
          {!focus && (
            <g className={`sys-hair${hover ? " off" : ""}`}>
              {lines.map((d) => (
                <path key={d} d={d} />
              ))}
            </g>
          )}
          {/* Keyed by the focus so its links fade in once the nodes have moved. */}
          <g key={p.focus ?? "rest"} className={focus ? "sys-flinks" : undefined}>
            {links.map((l) => {
              const a = boxOf(l[0]);
              const b = boxOf(l[1]);
              if (!a || !b) return null;
              const c = stroke(l);
              const faint = focus && hover && hover !== p.focus && !l.includes(hover);
              return (
                <path
                  key={l.join(">")}
                  className={`sys-edge${faint ? " faint" : ""}`}
                  d={wirePath(a, b)}
                  fill="none"
                  style={{ stroke: c }}
                  strokeWidth={2}
                  markerEnd={`url(#sym-arrow-${marker(c)})`}
                />
              );
            })}
          </g>
        </svg>
        {placed.chips.map((c) => (
          <button
            key={c.key}
            type="button"
            className={nodeClass(c.key, ["chip"])}
            style={place(boxOf(c.key))}
            {...nodeEvents(c.key)}
          >
            <span className="sys-sw" style={{ background: compColor(c.component) }} />
            <span className="sys-sym-nm">{compName(c.component)}</span>
          </button>
        ))}
        {placed.syms.map((b) => {
          const out = b.side !== "in";
          return (
            <button
              key={b.key}
              type="button"
              className={nodeClass(b.key, [
                out && "out",
                out && !focus?.pos.has(b.key) && "gone",
                status.get(b.sym.id),
              ])}
              style={place(boxOf(b.key))}
              title={b.sym.signature}
              {...nodeEvents(b.key)}
              onDoubleClick={() => p.onFile(b.sym.path, b.sym.line)}
            >
              <span className={`sys-k ${b.sym.kind}`}>{KIND_LABEL[b.sym.kind]}</span>
              <span className="sys-sym-nm">
                {out && <span className="sys-sym-comp">{compName(b.sym.component)} · </span>}
                {b.sym.name}
              </span>
            </button>
          );
        })}
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
  );
}
