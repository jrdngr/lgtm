import { getCurrentWindow } from "@tauri-apps/api/window";
import type { IDockviewHeaderActionsProps } from "dockview-react";
import { createContext, useContext, useEffect, useState } from "react";
import type { WorkspaceName } from "../core/workspace";
import { JjBusyTag } from "./JjBusy";
import { SideToggle } from "./SideToggle";

/** The window has no title bar of its own: the top row of the panels is it. macOS draws the
 * traffic lights over its left end. */
interface TitleBar {
  leftOpen: boolean;
  toggleLeft(): void;
  workspace: WorkspaceName;
}

export const TitleBarContext = createContext<TitleBar | null>(null);

/** How far down the window the side panels' headers reach. */
const TOP_ROW = 31;

/** Room for the traffic lights, then the left panel's toggle. Sits at the start of the
 * Workspaces header, or of the top-left pane's tabs while the left panel is hidden. */
export function TitleLead() {
  const bar = useContext(TitleBarContext);
  if (!bar) return null;
  return (
    <div className="title-lead">
      <span className="lights-room" />
      <SideToggle side="left" open={bar.leftOpen} shortcut="⌘B" onClick={bar.toggleLeft} />
      <JjBusyTag workspace={bar.workspace} />
    </div>
  );
}

/** A main dock pane's tabs start with `TitleLead` when the pane is in the window's top-left
 * corner, which it is while the left panel is hidden. */
export function TabsLead({ group }: IDockviewHeaderActionsProps) {
  const bar = useContext(TitleBarContext);
  const [corner, setCorner] = useState(false);

  useEffect(() => {
    const el = group.element;
    const check = () => {
      const dock = el.closest(".dock")?.getBoundingClientRect();
      const r = el.getBoundingClientRect();
      setCorner(!!dock && r.width > 0 && r.left - dock.left < 2 && r.top - dock.top < 2);
    };
    check();
    const seen = new ResizeObserver(check);
    seen.observe(el);
    const dock = el.closest(".dock");
    if (dock) seen.observe(dock);
    return () => seen.disconnect();
  }, [group]);

  return corner && bar && !bar.leftOpen ? <TitleLead /> : null;
}

/** Whether the window is full screen, where macOS hides the traffic lights. The top row's
 * empty spots move the window, and double-clicking them zooms it, as a title bar's do. */
export function useTitleBar() {
  const [fullscreen, setFullscreen] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    const check = () => {
      win.isFullscreen().then(setFullscreen, () => {});
    };
    check();
    const resized = win.onResized(check);
    // Taken before dockview's pointer drag sees it, which would move the pane instead.
    const onPointer = (e: PointerEvent) => {
      if (inTopRow(e)) e.stopPropagation();
    };
    const onDown = (e: MouseEvent) => {
      if (!inTopRow(e)) return;
      e.preventDefault();
      if (e.detail === 2) win.toggleMaximize();
      else win.startDragging();
    };
    window.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("mousedown", onDown, true);
    return () => {
      resized.then((stop) => stop());
      window.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("mousedown", onDown, true);
    };
  }, []);

  return fullscreen;
}

/** An empty spot in the top row: the side panels' headers, or a top pane's tab strip. */
function inTopRow(e: MouseEvent) {
  const target = e.target as Element;
  if (e.button !== 0 || target.closest(INTERACTIVE)) return false;
  const strip = target.closest(".dv-tabs-and-actions-container");
  if (strip) return strip.getBoundingClientRect().top < 1;
  return e.clientY < TOP_ROW;
}

const INTERACTIVE =
  "button, a, input, select, textarea, [role=tab], [contenteditable], .dv-tab, .cm-editor, .xterm, .side-resizer";
