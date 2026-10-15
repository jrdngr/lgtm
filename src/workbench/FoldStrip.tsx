import type { DockviewApi, DockviewGroupPanel } from "dockview-react";
import { useEffect, useReducer, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { fold, foldAxis, foldOf, neighbor, subscribeFolds, unfold } from "./fold";
import { activate } from "./panels";
import "./fold.css";

/** Re-renders on folds and on the group's tabs changing. */
function useGroup(api: DockviewApi, group: DockviewGroupPanel) {
  useSyncExternalStore(subscribeFolds, () => foldOf(api, group));
  const [, tick] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    const subs = [
      api.onDidAddPanel(tick),
      api.onDidRemovePanel(tick),
      api.onDidLayoutChange(tick),
      ...group.panels.map((p) => p.api.onDidTitleChange(tick)),
    ];
    return () => {
      for (const s of subs) s.dispose();
    };
  }, [api, group, group.panels]);
}

/** Two arrows at a center line: pointing in to collapse, out to expand. */
function FoldIcon({ out }: { out?: boolean }) {
  const heads = out
    ? "M4 5.5 1.5 8 4 10.5M12 5.5 14.5 8 12 10.5"
    : "M4 5.5 6.5 8 4 10.5M12 5.5 9.5 8 12 10.5";
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" className="fold-icon">
      <g fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round">
        <path d="M8 2.5v11M1.5 8h5M9.5 8h5" />
        <path d={heads} strokeLinejoin="round" />
      </g>
    </svg>
  );
}

/** The group's collapse button, and the strip it collapses to: every tab's title, which
 * opens the group on that tab. */
export function Fold({
  containerApi: api,
  group,
}: {
  containerApi: DockviewApi;
  group: DockviewGroupPanel;
}) {
  useGroup(api, group);
  const folded = foldOf(api, group);
  return (
    <>
      <button
        type="button"
        title="Collapse"
        aria-label="Collapse"
        disabled={!foldAxis(api, group)}
        onClick={() => {
          fold(api, group);
          // Keys go to a neighbor rather than the hidden tabs.
          const next = (["right", "left", "down", "up"] as const)
            .map((d) => neighbor(api, group, d))
            .find((g) => g && !foldOf(api, g));
          if (next?.activePanel) activate(next.activePanel);
        }}
      >
        <FoldIcon />
      </button>
      {folded &&
        createPortal(
          // biome-ignore lint/a11y/useKeyWithClickEvents: each title is a button; the rest of the strip is extra room to click
          // biome-ignore lint/a11y/noStaticElementInteractions: as above
          <div
            className={`fold-strip ${folded.axis}`}
            onClick={(e) => e.target === e.currentTarget && unfold(api, group)}
          >
            <button
              type="button"
              className="fold-open"
              title="Expand"
              aria-label="Expand"
              onClick={() => unfold(api, group)}
            >
              <FoldIcon out />
            </button>
            {group.panels.map((p) => (
              <button
                type="button"
                key={p.id}
                className="fold-tab"
                onClick={() => {
                  unfold(api, group);
                  activate(p);
                }}
              >
                <span>{p.title}</span>
              </button>
            ))}
          </div>,
          group.element,
        )}
    </>
  );
}
