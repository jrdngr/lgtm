import { useMemo } from "react";
import type { SystemDto } from "../bindings";
import { tint } from "./colors";
import { layout } from "./layout";
import { sceneOf } from "./scene";

const W = 176;
const H = 112;
const PAD = 10;

interface Props {
  map: SystemDto;
  /** The container shown. */
  container: string;
  onPick(container: string): void;
}

/** The system's containers in small, with the one shown outlined. */
export function Minimap({ map, container, onPick }: Props) {
  const boxes = useMemo(() => {
    const placed = layout(sceneOf(map, { kind: "containers" })).nodes.filter(
      (n) => n.kind === "container" && n.path !== undefined,
    );
    if (!placed.length) return [];
    const x0 = Math.min(...placed.map((n) => n.x));
    const y0 = Math.min(...placed.map((n) => n.y));
    const w = Math.max(...placed.map((n) => n.x + n.w)) - x0;
    const h = Math.max(...placed.map((n) => n.y + n.h)) - y0;
    const k = Math.min((W - PAD * 2) / w, (H - PAD * 2) / h);
    const ox = (W - w * k) / 2;
    const oy = (H - h * k) / 2;
    return placed.map((n) => ({
      id: n.path ?? "",
      name: n.name,
      color: n.color ?? "#8b8d93",
      drill: !!n.tiles?.length,
      x: ox + (n.x - x0) * k,
      y: oy + (n.y - y0) * k,
      w: n.w * k,
      h: n.h * k,
    }));
  }, [map]);
  return (
    <div
      className="sys-mini"
      style={{ width: W, height: H }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      {boxes.map((b) => (
        <button
          key={b.id}
          type="button"
          aria-label={b.name}
          className={b.id === container ? "on" : undefined}
          disabled={!b.drill}
          style={{
            left: b.x,
            top: b.y,
            width: b.w,
            height: b.h,
            background: tint(b.color, b.id === container ? 0.55 : 0.28),
          }}
          onClick={() => onPick(b.id)}
        />
      ))}
    </div>
  );
}
