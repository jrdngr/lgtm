import type { CSSProperties } from "react";
import { NEUTRAL, tint } from "./colors";
import { DeltaCounts } from "./Delta";
import { Chevron, Person } from "./glyphs";
import type { Box, Placed } from "./layout";

/** A tile's width for a component of `files` files. */
const tileWidth = (files: number) => Math.round(Math.min(40, 8 + Math.sqrt(files) * 5));

const STORE = /sql|database|\bdb\b|store|cache|redis/i;

/** A node's content. */
export function Node({ n, stale }: { n: Placed; stale: boolean }) {
  const color = n.color ?? NEUTRAL;
  const swatch = n.color && <span className="sys-sw" style={{ background: n.color }} />;
  return (
    <>
      {n.kind === "person" && <Person />}
      <span className="sys-hd">
        {n.kind === "system" && <span className="sys-sw round" style={{ background: NEUTRAL }} />}
        {n.kind !== "component" && !n.door && swatch}
        <span className="sys-nm">{n.name}</span>
        {(n.kind === "external" || (n.kind === "container" && (n.tech?.length ?? 0) <= 10)) &&
          n.tech && <span className="sys-chip">{n.tech}</span>}
        {n.door && <Chevron />}
        {stale && <span className="sys-stale" />}
        {n.kind === "component" && n.status === "added" && (
          <span className="sys-delta new">new</span>
        )}
        {n.kind === "component" && n.status !== "added" && (
          <DeltaCounts d={n.delta} className="sys-delta" />
        )}
        {n.kind === "container" && !n.door && (
          <DeltaCounts d={n.delta} className="sys-delta compact-only" />
        )}
      </span>
      {n.kind === "container" && !n.door && <DeltaCounts d={n.delta} className="sys-cnt" />}
      {n.flag && <span className="sys-flag">!</span>}
      {n.kind === "container" && !n.door && (n.tech?.length ?? 0) > 10 && (
        <span className="sys-chip">{n.tech}</span>
      )}
      {!!n.tiles?.length && (
        <span className="sys-tiles">
          {n.tiles.map((t, i) => (
            <span
              // biome-ignore lint/suspicious/noArrayIndexKey: tiles have no identity
              key={i}
              className={`sys-tile${t.changed ? " c" : ""}`}
              style={{ width: tileWidth(t.files), background: tint(color, 0.6) }}
            />
          ))}
        </span>
      )}
      {!!n.tiles?.length && (
        <span className="sys-go" style={{ background: color }}>
          <Chevron />
        </span>
      )}
      {n.blocks && (
        <span className="sys-blocks">
          {n.blocks.map((b) => (
            <span key={b.name} className="sys-block" style={{ background: tint(b.color, 0.35) }}>
              {b.name}
            </span>
          ))}
        </span>
      )}
    </>
  );
}

/** Where a node sits: its place on the map, or in the collapsed graph, or none when the
 * graph leaves it out. */
export type Spot = { box: Box; compact: boolean } | undefined;

export function nodeClass(n: Placed, spot: Spot, lit: Set<string> | undefined) {
  const store = n.kind === "container" && !n.tiles?.length && !!n.tech && STORE.test(n.tech);
  return [
    "sys-node",
    n.kind,
    n.door && "door",
    store && "store",
    n.status,
    spot?.compact && "compact",
    !spot && "gone",
    lit && !lit.has(n.id) && "dim",
  ]
    .filter(Boolean)
    .join(" ");
}

export function nodeStyle(n: Placed, spot: Spot, selected: boolean, lit: boolean): CSSProperties {
  const color = n.color ?? NEUTRAL;
  const b = spot?.box ?? n;
  const style: CSSProperties = {
    transform: `translate(${b.x}px, ${b.y}px)`,
    width: b.w,
    height: b.h,
  };
  if (selected) {
    style.border = `1.5px solid ${color}`;
    style.boxShadow = `0 0 0 4px ${tint(color, 0.18)}, 0 8px 20px rgb(0 0 0 / 28%)`;
    if (n.kind === "component") style.background = tint(color, 0.16);
  } else if (lit && n.kind === "component") {
    style.borderColor = tint(color, 0.45);
    style.background = tint(color, 0.07);
  } else if (n.door) {
    style.borderColor = tint(color, 0.6);
    style.background = tint(color, 0.08);
    style.color = color;
  }
  return style;
}
