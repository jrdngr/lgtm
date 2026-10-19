import { useEffect, useRef } from "react";
import type { SystemDto, TourDto } from "../bindings";
import { PipeButton } from "../terminal/PipeButton";
import { NEUTRAL, tint } from "./colors";
import { Play } from "./glyphs";
import { containersOf } from "./scene";
import { tourPath } from "./tours";

interface Props {
  map: SystemDto;
  colors: Map<string, string>;
  /** The container in view, whose components without a tour offer Write Tour. */
  container?: string;
  onStart(tour: TourDto): void;
  /** Asks a terminal agent to write the tour of a container or component, by id. */
  onWrite(id: string): void;
  onClose(): void;
}

/** Every tour: the system's, then each container's and its components'. */
export function ToursMenu({ map, colors, container, onStart, onWrite, onClose }: Props) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const away = (e: PointerEvent) => {
      const at = e.target as Element;
      if (!root.current?.contains(at) && !at.closest?.("[data-tours]")) onClose();
    };
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onClose]);

  const byId = new Map(map.tours.map((t) => [t.id, t]));
  const system = map.tours.filter((t) => t.id === "overview" || t.id === "features");
  const groups = containersOf(map)
    .filter((c) => c.id !== "" && c.files > 0)
    .map((c) => {
      const comps = map.components.filter((k) => k.container === c.id);
      return {
        c,
        own: byId.get(`c:${c.id}`),
        comps: comps.filter((k) => byId.has(`k:${k.id}`) || c.id === container),
      };
    });

  const row = (t: TourDto | undefined, id: string, name: string, color: string) =>
    t ? (
      <button
        key={id}
        type="button"
        className="sys-trow"
        role="menuitem"
        onClick={() => onStart(t)}
      >
        <span className="sys-tic" style={{ background: tint(color, 0.16), color }}>
          <Play />
        </span>
        <span className="sys-tn2">
          <span className="sys-tt2">{t.title || name}</span>
          <span className="sys-tpath">{tourPath(map, colors, t)}</span>
        </span>
        <span className="sys-tcount">{t.steps.length} steps</span>
      </button>
    ) : (
      <div key={id} className="sys-trow">
        <span className="sys-tic none">
          <Play />
        </span>
        <span className="sys-tn2">
          <span className="sys-tt2 none">{name}</span>
        </span>
        <PipeButton className="sm" onClick={() => onWrite(id)}>
          Write Tour
        </PipeButton>
      </div>
    );

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: keeps clicks off the canvas under it
    <div
      className="sys-pop"
      role="menu"
      aria-label="Tours"
      ref={root}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
    >
      {!!system.length && (
        <>
          <span className="sys-pg">System</span>
          {system.map((t) => row(t, t.id, t.title, NEUTRAL))}
        </>
      )}
      {groups.map(({ c, own, comps }) => {
        const color = colors.get(c.id) ?? NEUTRAL;
        return (
          <div key={c.id}>
            <div className="sys-psep" />
            <span className="sys-pg">
              <span className="sys-sw" style={{ background: color }} />
              {c.name}
            </span>
            {row(own, `c:${c.id}`, c.name, color)}
            {comps.map((k) => row(byId.get(`k:${k.id}`), `k:${k.id}`, k.name, color))}
          </div>
        );
      })}
    </div>
  );
}
