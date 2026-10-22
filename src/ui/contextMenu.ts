import { LogicalPosition } from "@tauri-apps/api/dpi";
import { Image } from "@tauri-apps/api/image";
import {
  type IconMenuItemOptions,
  Menu,
  type MenuItemOptions,
  type SubmenuOptions,
} from "@tauri-apps/api/menu";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { MouseEvent as ReactMouseEvent } from "react";
import { logger } from "../core/log";

const log = logger("context menu");

/** One row of a right-click menu. `false` and `undefined` drop out, so items can be conditional. */
export type CtxItem =
  | {
      label: string;
      onSelect(): void;
      /** As macOS writes it: `⌘S`, `⇧⌘T`, `x`, `↵`. Shown only; the app's own keys still run it. */
      keys?: string;
      disabled?: boolean;
      checked?: boolean;
      /** A color dot before the label, as `#rrggbb`, ringed when `checked`. */
      swatch?: string;
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
  | IconMenuItemOptions
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

/** Menu images draw 18pt tall, so a 10pt dot sits in a 36px square for Retina. */
const SWATCH_PX = 36;
const SWATCH_R = 10;

const RING_IN = 13;
const RING_OUT = 16;

/** A filled circle, antialiased at its edge, with a ring around it when `ringed`. */
export function swatchPixels(hex: string, ringed = false): Uint8Array {
  const [r, g, b] = [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));
  const px = new Uint8Array(SWATCH_PX * SWATCH_PX * 4);
  const c = SWATCH_PX / 2;
  for (let y = 0; y < SWATCH_PX; y++) {
    for (let x = 0; x < SWATCH_PX; x++) {
      const d = Math.hypot(x + 0.5 - c, y + 0.5 - c);
      const edge = (r: number) => Math.max(0, Math.min(1, r + 0.5 - d));
      const ring = ringed ? edge(RING_OUT) - edge(RING_IN) : 0;
      const a = Math.max(edge(SWATCH_R), ring);
      px.set([r, g, b, Math.round(a * 255)], (y * SWATCH_PX + x) * 4);
    }
  }
  return px;
}

const swatches = new Map<string, Promise<Image>>();

function swatch(hex: string, ringed: boolean): Promise<Image> {
  const key = `${hex}${ringed ? "o" : ""}`;
  let img = swatches.get(key);
  if (!img) {
    img = Image.new(swatchPixels(hex, ringed), SWATCH_PX, SWATCH_PX);
    swatches.set(key, img);
    img.catch(() => swatches.delete(key));
  }
  return img;
}

function native(items: CtxItem[]): Promise<Native[]> {
  return Promise.all(
    items.map(async (i): Promise<Native> => {
      if (i === "-") return { item: "Separator" };
      if ("items" in i) {
        return { text: i.label, enabled: !i.disabled, items: await native(tidy(i.items)) };
      }
      const base = {
        text: i.label,
        enabled: !i.disabled,
        accelerator: i.keys ? accelerator(i.keys) : undefined,
        action: () => i.onSelect(),
      };
      if (i.swatch) return { ...base, icon: await swatch(i.swatch, !!i.checked) };
      return i.checked === undefined ? base : { ...base, checked: i.checked };
    }),
  );
}

/** Shows a native menu at the pointer. Stops the event, so no other menu opens for it. */
export function openMenu(e: ReactMouseEvent | MouseEvent, entries: CtxEntry[]) {
  e.preventDefault();
  e.stopPropagation();
  const items = tidy(entries);
  if (!items.length) return;
  const at = new LogicalPosition(e.clientX, e.clientY);
  native(items)
    .then((n) => Menu.new({ items: n as MenuItemOptions[] }))
    .then((menu) => menu.popup(at))
    .catch((err) => log.error("context menu", err));
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
      .catch((err) => log.error("context menu", err));
  });
}

/** Copies text to the clipboard, resolving once it's there. Goes through the app rather than
 * the web view, which refuses clipboard access outside a click or key press. */
export const copy = (text: string): Promise<void> =>
  writeText(text).catch((e) => log.error("copy", e));

/** The clipboard's text, empty when it holds none. */
export const clipboardText = (): Promise<string> => readText().catch(() => "");
