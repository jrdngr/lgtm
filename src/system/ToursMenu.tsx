import { useEffect, useRef } from "react";
import type { SystemDto, TourDto } from "../bindings";
import { PipeButton } from "../terminal/PipeButton";
import { openMenu } from "../ui/contextMenu";
import { Lifted } from "../ui/Lifted";
import { NEUTRAL } from "./colors";
import { Play } from "./glyphs";
import { containersOf } from "./scene";
import { tourPath } from "./tours";

interface Props {
  map: SystemDto;
  colors: Map<string, string>;
  /** The container in view, whose components without a tour offer Write Tour. */
  container?: string;
  onStart(tour: TourDto): void;
  /** Asks a terminal agent to write a tour, by id: `overview`, `features`, or a container's
   * or component's. */
  onWrite(id: string): void;
  onClose(): void;
}

/** Every tour, each with Play or Write Tour: the system's, then each container's and its
 * components'. Right-click one to rewrite it. */
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

  const row = (t: TourDto | undefined, id: string, name: string, color: string) => (
    <div
      key={id}
      className="sys-trow"
      onContextMenu={(e) =>
        t ? openMenu(e, [{ label: "Rewrite Tour", onSelect: () => onWrite(id) }]) : undefined
      }
    >
      <span className="sys-tdot" style={{ background: color }} />
      <span className="sys-tn2">
        <span className={`sys-tt2${t ? "" : " none"}`}>{t?.title || name}</span>
        {t && <span className="sys-tpath">{tourPath(map, colors, t)}</span>}
      </span>
      {t ? (
        <button type="button" className="sys-tplay" onClick={() => onStart(t)}>
          <Play size={10} />
          Play
          <span className="sys-tplay-n">{t.steps.length}</span>
        </button>
      ) : (
        <PipeButton background className="sm" onClick={() => onWrite(id)}>
          Write Tour
        </PipeButton>
      )}
    </div>
  );

  return (
    <Lifted
      className="sys-pop"
      role="menu"
      aria-label="Tours"
      ref={root}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
    >
      <span className="sys-pg">System</span>
      {row(byId.get("overview"), "overview", "Overview", NEUTRAL)}
      {row(byId.get("features"), "features", "Every feature", NEUTRAL)}
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
    </Lifted>
  );
}
