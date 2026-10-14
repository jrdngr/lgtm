import { LogicalPosition } from "@tauri-apps/api/dpi";
import { Menu, type MenuItemOptions, type SubmenuOptions } from "@tauri-apps/api/menu";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { MouseEvent as ReactMouseEvent } from "react";

/** One row of a right-click menu. `false` and `undefined` drop out, so items can be conditional. */
export type CtxItem =
  | {
      label: string;
      onSelect(): void;
      /** As macOS writes it: `⌘S`, `⇧⌘T`, `x`, `↵`. Shown only; the app's own keys still run it. */
      keys?: string;
      disabled?: boolean;
      checked?: boolean;
    }
  | { label: string; items: CtxEntry[]; disabled?: boolean }
  | "-";
export type CtxEntry = CtxItem | false | "" | undefined | null;

const MODS: Record<string, string> = { "⌘": "Cmd", "⇧": "Shift", "⌥": "Alt", "⌃": "Ctrl" };
const NAMED: Record<string, string> = { "↵": "Enter", "`": "Backquote", "⌫": "Backspace" };

/** `⇧⌘T` as an accelerator string, `Shift+Cmd+T`. Undefined for what a menu can't show,
 * like the two-key `za`, so the item shows without a shortcut. */
export function accelerator(keys: string): string | undefined {
  const mods: string[] = [];
  let rest = keys;
  while (rest && MODS[rest[0]]) {
    mods.push(MODS[rest[0]]);
    rest = rest.slice(1);
  }
  if ([...rest].length !== 1) return undefined;
  let key = NAMED[rest] ?? rest;
  if (/^[A-Z]$/.test(key) && !mods.includes("Shift")) mods.unshift("Shift");
  key = key.toUpperCase();
  return [...mods, key].join("+");
}

type Native =
  | MenuItemOptions
  | SubmenuOptions
  | { item: "Separator" }
  | { text: string; checked: boolean };

/** Drops hidden entries, doubled separators, and separators at either end. */
export function tidy(entries: CtxEntry[]): CtxItem[] {
  const out: CtxItem[] = [];
  for (const e of entries) {
    if (!e) continue;
    if (e === "-" && (out.length === 0 || out[out.length - 1] === "-")) continue;
    out.push(e);
  }
  while (out[out.length - 1] === "-") out.pop();
  return out;
}

function native(items: CtxItem[]): Native[] {
  return items.map((i): Native => {
    if (i === "-") return { item: "Separator" };
    if ("items" in i) return { text: i.label, enabled: !i.disabled, items: native(tidy(i.items)) };
    const base = {
      text: i.label,
      enabled: !i.disabled,
      accelerator: i.keys ? accelerator(i.keys) : undefined,
      action: () => i.onSelect(),
    };
    return i.checked === undefined ? base : { ...base, checked: i.checked };
  });
}

/** Shows a native menu at the pointer. Stops the event, so no other menu opens for it. */
export function openMenu(e: ReactMouseEvent | MouseEvent, entries: CtxEntry[]) {
  e.preventDefault();
  e.stopPropagation();
  const items = tidy(entries);
  if (!items.length) return;
  const at = new LogicalPosition(e.clientX, e.clientY);
  Menu.new({ items: native(items) as MenuItemOptions[] })
    .then((menu) => menu.popup(at))
    .catch((err) => console.error("context menu", err));
}

/** A handler for `onContextMenu` that builds its items when the menu opens. */
export const contextMenu = (entries: () => CtxEntry[]) => (e: ReactMouseEvent | MouseEvent) =>
  openMenu(e, entries());

const editable = (t: EventTarget | null) =>
  t instanceof HTMLElement &&
  t.closest("input, textarea, [contenteditable=''], [contenteditable='true']") !== null;

/** Replaces the web view's menu: text fields get Cut, Copy, Paste and Select All, anything
 * without its own menu gets none. ⌥-right-click keeps the web view's menu in dev builds. */
export function installContextMenus() {
  window.addEventListener("contextmenu", (e) => {
    if (e.defaultPrevented) return;
    if (import.meta.env.DEV && e.altKey) return;
    e.preventDefault();
    if (!editable(e.target)) return;
    const at = new LogicalPosition(e.clientX, e.clientY);
    Menu.new({
      items: [
        { item: "Cut" },
        { item: "Copy" },
        { item: "Paste" },
        { item: "Separator" },
        { item: "SelectAll" },
      ],
    })
      .then((menu) => menu.popup(at))
      .catch((err) => console.error("context menu", err));
  });
}

/** Copies text to the clipboard, resolving once it's there. Goes through the app rather than
 * the web view, which refuses clipboard access outside a click or key press. */
export const copy = (text: string): Promise<void> =>
  writeText(text).catch((e) => console.error("copy", e));

/** The clipboard's text, empty when it holds none. */
export const clipboardText = (): Promise<string> => readText().catch(() => "");
