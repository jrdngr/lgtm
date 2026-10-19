import type { IDockviewHeaderActionsProps } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { addToGroup, KINDS, kindTitle, splitGroup } from "../core/panels";
import { useWorkspace } from "../core/workspace";
import { openMenu } from "../ui/contextMenu";
import { Fold } from "./FoldStrip";
import { canMaximize, toggleMaximize, useMaximized } from "./maximize";
import { stripMenu } from "./tabMenu";
import "../review/MarkMenu.css";
import "./GroupActions.css";

/** A tab after a group's tabs that opens a menu of the kinds of tab to add there. */
export function NewTab({ containerApi, group }: IDockviewHeaderActionsProps) {
  const [at, setAt] = useState<{ x: number; y: number }>();
  const close = () => setAt(undefined);
  const ref = useRef<HTMLButtonElement>(null);
  useStripMenu(ref, containerApi, group);
  return (
    <>
      <button
        ref={ref}
        type="button"
        className="new-tab"
        title="New tab"
        aria-label="New tab"
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          setAt({ x: r.left, y: r.bottom });
        }}
      >
        +
      </button>
      {at &&
        // In the body, so the group's header doesn't clip it.
        createPortal(
          <>
            <div className="menu-backdrop new-tab-backdrop" onMouseDown={close} />
            <div className="mark-menu new-tab-menu" style={{ left: at.x, top: at.y }}>
              {KINDS.map((kind) => (
                <button
                  type="button"
                  key={kind}
                  onClick={() => {
                    close();
                    addToGroup(containerApi, group, kind);
                  }}
                >
                  {kindTitle(kind)}
                </button>
              ))}
            </div>
          </>,
          document.body,
        )}
    </>
  );
}

/** Gives the pane's header, outside its tabs, the strip menu. Dockview draws the header, so the
 * listener goes on its element; it runs before a tab's own React handler, so tabs are left to
 * that. */
function useStripMenu(
  inside: React.RefObject<HTMLElement | null>,
  api: IDockviewHeaderActionsProps["containerApi"],
  group: IDockviewHeaderActionsProps["group"],
) {
  const workspace = useWorkspace();
  useEffect(() => {
    const header = inside.current?.closest(".dv-tabs-and-actions-container");
    if (!header) return;
    const onMenu = (e: Event) => {
      const target = e.target as HTMLElement;
      if (target.closest(".dv-tab")) return;
      const inBottom = !!target.closest(".bottom-panel");
      openMenu(e as MouseEvent, stripMenu({ workspace, api, group, inBottom }));
    };
    const onDouble = (e: Event) => {
      const target = e.target as HTMLElement;
      if (target.closest(".dv-tab, button, .bottom-panel")) return;
      toggleMaximize(api, group);
    };
    header.addEventListener("contextmenu", onMenu);
    header.addEventListener("dblclick", onDouble);
    return () => {
      header.removeEventListener("contextmenu", onMenu);
      header.removeEventListener("dblclick", onDouble);
    };
  }, [inside, api, group, workspace]);
}

/** Two rects side by side, or stacked. */
function SplitIcon({ stacked }: { stacked: boolean }) {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
      <rect x="1.5" y="2.5" width="13" height="11" rx="1" fill="none" stroke="currentColor" />
      {stacked ? (
        <line x1="1.5" y1="8" x2="14.5" y2="8" stroke="currentColor" />
      ) : (
        <line x1="8" y1="2.5" x2="8" y2="13.5" stroke="currentColor" />
      )}
    </svg>
  );
}

/** Buttons that split the group, right or down, with an empty pane. */
export function SplitButtons({ containerApi, group }: IDockviewHeaderActionsProps) {
  return (
    <div className="split-buttons">
      <button
        type="button"
        title="Split right"
        aria-label="Split right"
        onClick={() => splitGroup(containerApi, group, "right")}
      >
        <SplitIcon stacked={false} />
      </button>
      <button
        type="button"
        title="Split down"
        aria-label="Split down"
        onClick={() => splitGroup(containerApi, group, "below")}
      >
        <SplitIcon stacked />
      </button>
      <Fold containerApi={containerApi} group={group} />
      <Maximize containerApi={containerApi} group={group} />
    </div>
  );
}

/** Arrows out to the corners, or in from them. */
function MaximizeIcon({ restore }: { restore: boolean }) {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
      <path
        d={
          restore
            ? "M13 7H9V3M9 7l4.5-4.5M3 9h4v4M7 9l-4.5 4.5"
            : "M9.5 2.5h4v4M13.5 2.5 9 7M6.5 13.5h-4v-4M2.5 13.5 7 9"
        }
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** Fills the main dock with the group, or restores the panes it hid. Not in the bottom panel. */
function Maximize({
  containerApi,
  group,
}: Pick<IDockviewHeaderActionsProps, "containerApi" | "group">) {
  useMaximized(containerApi);
  if (group.element.closest(".bottom-panel")) return null;
  const on = group.api.isMaximized();
  const label = on ? "Restore" : "Maximize";
  return (
    <button
      type="button"
      className={on ? "on" : undefined}
      title={label}
      aria-label={label}
      disabled={!canMaximize(containerApi, group)}
      onClick={() => toggleMaximize(containerApi, group)}
    >
      <MaximizeIcon restore={on} />
    </button>
  );
}
