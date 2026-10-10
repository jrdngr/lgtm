import type { MarkState } from "../bindings";

export type MenuAction = MarkState | "clear" | "comment" | "change" | "agent" | "copy";

export const MENU_KEYS: Record<string, MenuAction> = {
  r: "reviewed",
  s: "skipped",
  f: "flagged",
  u: "clear",
  c: "comment",
  g: "change",
  a: "agent",
};

const ITEMS: { action: MenuAction; key?: string; label: string }[] = [
  { action: "reviewed", key: "r", label: "Reviewed" },
  { action: "skipped", key: "s", label: "Skip" },
  { action: "flagged", key: "f", label: "Flag" },
  { action: "clear", key: "u", label: "Clear mark" },
  { action: "comment", key: "c", label: "Comment" },
  { action: "change", key: "g", label: "Change with agent" },
  { action: "agent", key: "a", label: "Send to agent" },
  { action: "copy", label: "Copy" },
];

interface Props {
  x: number;
  y: number;
  changed: number;
  onPick(action: MenuAction): void;
  onClose(): void;
}

export function MarkMenu({ x, y, changed, onPick, onClose }: Props) {
  return (
    <>
      <div
        className="menu-backdrop"
        onMouseDown={onClose}
        onContextMenu={(e) => e.preventDefault()}
      />
      <div className="mark-menu" style={{ left: x, top: y }}>
        <div className="menu-count">
          {changed} changed {changed === 1 ? "row" : "rows"}
        </div>
        {ITEMS.map((item) => (
          <button type="button" key={item.action} onClick={() => onPick(item.action)}>
            <span className="menu-key">{item.key ?? ""}</span>
            {item.label}
          </button>
        ))}
      </div>
    </>
  );
}
