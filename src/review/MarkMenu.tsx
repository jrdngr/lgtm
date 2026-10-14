import { useLayoutEffect, useRef, useState } from "react";
import type { MarkColor } from "../bindings";
import { colorForKey, type MarkSetup, markVar } from "./marks";
import { plural } from "./state";
import "./MarkMenu.css";

export type MenuAction = MarkColor | "clear" | "comment" | "change" | "agent" | "copy";

const LETTERS: Record<string, MenuAction> = {
  c: "comment",
  g: "change",
  a: "agent",
};

/** The menu item a key picks: a mark color's number, 0 to clear, or an action's letter. */
export function menuAction(key: string, setup: MarkSetup): MenuAction | undefined {
  if (key === "0") return "clear";
  return colorForKey(setup, key) ?? LETTERS[key];
}

const ACTIONS: { action: MenuAction; key?: string; label: string }[] = [
  { action: "clear", key: "0", label: "Clear mark" },
  { action: "comment", key: "c", label: "Comment" },
  { action: "change", key: "g", label: "Change with agent" },
  { action: "agent", key: "a", label: "Send to agent" },
  { action: "copy", label: "Copy" },
];

interface Props {
  x: number;
  y: number;
  changed: number;
  setup: MarkSetup;
  onPick(action: MenuAction): void;
  onClose(): void;
}

/** Shifts the menu back inside its pane when it would run past the bottom or right edge. */
function useKeptInside(x: number, y: number) {
  const ref = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState({ left: x, top: y });
  useLayoutEffect(() => {
    const menu = ref.current;
    const pane = menu?.offsetParent;
    if (!menu || !pane) return setPlace({ left: x, top: y });
    setPlace({
      left: Math.max(0, Math.min(x, pane.clientWidth - menu.offsetWidth)),
      top: Math.max(0, Math.min(y, pane.clientHeight - menu.offsetHeight)),
    });
  }, [x, y]);
  return { ref, place };
}

export function MarkMenu({ x, y, changed, setup, onPick, onClose }: Props) {
  const { ref, place } = useKeptInside(x, y);
  return (
    <>
      <div
        className="menu-backdrop"
        onMouseDown={onClose}
        onContextMenu={(e) => e.preventDefault()}
      />
      <div className="mark-menu" ref={ref} style={place}>
        <div className="menu-count">
          {changed} changed {plural(changed)}
        </div>
        {setup.shown.map((s, i) => (
          <button type="button" key={s.color} onClick={() => onPick(s.color)}>
            <span className="menu-key">{i + 1}</span>
            <span className="menu-swatch" style={{ background: markVar(s.color) }} />
            {setup.label(s.color)}
          </button>
        ))}
        {ACTIONS.map((item) => (
          <button type="button" key={item.action} onClick={() => onPick(item.action)}>
            <span className="menu-key">{item.key ?? ""}</span>
            {item.label}
          </button>
        ))}
      </div>
    </>
  );
}
