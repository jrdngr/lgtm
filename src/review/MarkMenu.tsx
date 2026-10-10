import type { MarkState } from "../bindings";

export type MenuAction = MarkState | "clear" | "comment" | "agent" | "copy";

export const MENU_KEYS: Record<string, MenuAction> = {
  r: "reviewed",
  s: "skipped",
  f: "flagged",
  u: "clear",
  c: "comment",
  a: "agent",
};

const ITEMS: { action: MenuAction; key?: string; label: string; disabled?: boolean }[] = [
  { action: "reviewed", key: "r", label: "Reviewed" },
  { action: "skipped", key: "s", label: "Skip" },
  { action: "flagged", key: "f", label: "Flag" },
  { action: "clear", key: "u", label: "Clear mark" },
  { action: "comment", key: "c", label: "Comment", disabled: true },
  { action: "agent", key: "a", label: "Send to agent" },
  { action: "copy", label: "Copy" },
];

export const DISABLED = new Set(ITEMS.filter((i) => i.disabled).map((i) => i.action));

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
          <button
            type="button"
            key={item.action}
            disabled={item.disabled}
            title={item.disabled ? "Not available yet" : undefined}
            onClick={() => onPick(item.action)}
          >
            <span className="menu-key">{item.key ?? ""}</span>
            {item.label}
          </button>
        ))}
      </div>
    </>
  );
}
