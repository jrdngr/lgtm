import { useLayoutEffect, useRef, useState } from "react";
import type { MarkColor } from "../bindings";
import type { CtxEntry } from "../ui/contextMenu";
import { colorForKey, type MarkSetup, markVar } from "./marks";
import { plural } from "./state";
import "./MarkMenu.css";

export type MenuAction =
  | MarkColor
  | "clear"
  | "comment"
  | "draft"
  | "agent"
  | "copy"
  | "location"
  | "open"
  | "collapse";

const LETTERS: Record<string, MenuAction> = {
  c: "comment",
  C: "draft",
  a: "agent",
  o: "open",
};

/** The menu item a key picks: a mark color's number, 0 to clear, or an action's letter. */
export function menuAction(key: string, setup: MarkSetup): MenuAction | undefined {
  if (key === "0") return "clear";
  return colorForKey(setup, key) ?? LETTERS[key];
}

type Item = { action: MenuAction; key?: string; label: string } | "-";

interface Props {
  x: number;
  y: number;
  changed: number;
  setup: MarkSetup;
  /** Shows Draft PR Comment, in a PR workspace. */
  canDraft: boolean;
  /** Shows Collapse Hunk, when the cursor is in a hunk. */
  inHunk: boolean;
  /** The symbol items on top, from the right-click word. */
  symbol?: CtxEntry[];
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

export function MarkMenu({
  x,
  y,
  changed,
  setup,
  canDraft,
  inHunk,
  symbol = [],
  onPick,
  onClose,
}: Props) {
  const { ref, place } = useKeptInside(x, y);
  const items: Item[] = [
    { action: "clear", key: "0", label: "Clear Mark" },
    "-",
    { action: "comment", key: "c", label: "Comment" },
    ...(canDraft ? [{ action: "draft", key: "C", label: "Draft PR Comment" } as const] : []),
    { action: "agent", key: "a", label: "Send to Agent" },
    "-",
    { action: "copy", key: "⌘C", label: "Copy" },
    { action: "location", label: "Copy with Location" },
    { action: "open", key: "o", label: "Open File at Line" },
    ...(inHunk ? [{ action: "collapse", key: "za", label: "Collapse Hunk" } as const] : []),
  ];
  return (
    <>
      <div
        className="menu-backdrop"
        onMouseDown={onClose}
        onContextMenu={(e) => e.preventDefault()}
      />
      <div className="mark-menu" ref={ref} style={place}>
        {symbol.map((item, i) =>
          !item || (item !== "-" && "items" in item) ? null : item === "-" ? (
            // biome-ignore lint/suspicious/noArrayIndexKey: separators have no identity
            <hr key={i} className="menu-sep" />
          ) : (
            <button
              type="button"
              key={item.label}
              disabled={item.disabled}
              onClick={() => {
                onClose();
                item.onSelect();
              }}
            >
              <span className="menu-key" />
              {item.label}
              {item.keys && <span className="menu-trail">{item.keys}</span>}
            </button>
          ),
        )}
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
        {items.map((item, i) =>
          item === "-" ? (
            // biome-ignore lint/suspicious/noArrayIndexKey: separators have no identity
            <hr key={i} className="menu-sep" />
          ) : (
            <button type="button" key={item.action} onClick={() => onPick(item.action)}>
              <span className="menu-key">{item.key ?? ""}</span>
              {item.label}
            </button>
          ),
        )}
      </div>
    </>
  );
}
