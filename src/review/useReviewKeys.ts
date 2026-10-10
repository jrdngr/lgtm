import { useRef } from "react";
import type { MarkState } from "../bindings";
import { isMac } from "../workbench/keys";
import { MENU_KEYS, type MenuAction } from "./MarkMenu";
import type { DisplayLine, FlatFile } from "./model";
import { expand, reviewSelection } from "./selection";
import type { SelectionApi } from "./useSelection";

interface Ctx {
  files: FlatFile[];
  lines: DisplayLine[];
  s: SelectionApi;
  menu: boolean;
  closeMenu(): void;
  pick(action: MenuAction): void;
  step(undo: boolean): void;
  copy(): void;
  applyMark(state: MarkState | null): void;
  openMenuAtCursor(): void;
  startComment(): void;
  openRevisions(): void;
  toggleFile(file: number): void;
  toggleHunk(file: number, hunk: number): void;
  toggleChecked(file: number): void;
}

/** Form fields keep their own keys. */
const isEditable = (t: EventTarget) =>
  t instanceof HTMLInputElement ||
  t instanceof HTMLSelectElement ||
  t instanceof HTMLTextAreaElement;

/** The review's keyboard handler. */
export function useReviewKeys(ctx: Ctx): (e: React.KeyboardEvent) => void {
  const pendingZ = useRef(false);
  const { files, lines, s } = ctx;
  const { cursor, sel, select } = s;

  return (e) => {
    if (isEditable(e.target)) return;
    if (ctx.menu) {
      e.preventDefault();
      if (e.key === "Escape") ctx.closeMenu();
      else if (MENU_KEYS[e.key] && !e.repeat) ctx.pick(MENU_KEYS[e.key]);
      return;
    }
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod) {
      const key = e.key.toLowerCase();
      if (key === "a") select(reviewSelection(lines));
      else if (key === "z") ctx.step(!e.shiftKey);
      else if (key === "c") {
        // Selected code text copies natively; otherwise copy the selected rows.
        if (window.getSelection()?.isCollapsed === false) return;
        ctx.copy();
      } else return;
      e.preventDefault();
      return;
    }
    if (pendingZ.current && e.key === "a") {
      pendingZ.current = false;
      const line = lines[cursor];
      if (line?.kind === "file") ctx.toggleFile(line.file);
      else if (line) ctx.toggleHunk(line.file, line.hunk);
      e.preventDefault();
      return;
    }
    pendingZ.current = e.key === "z";
    switch (e.key) {
      case "j":
      case "ArrowDown":
        s.move(1, s.extending);
        break;
      case "k":
      case "ArrowUp":
        s.move(-1, s.extending);
        break;
      case "J":
      case "K":
        s.extend(e.key === "J" ? 1 : -1);
        break;
      case "v":
        s.toggleExtend();
        break;
      case "e": {
        const next = expand(files, lines, cursor, sel);
        if (next) s.grow(next);
        break;
      }
      case "E":
        s.shrink();
        break;
      case "Escape":
        s.collapse();
        break;
      case " ":
        if (!e.repeat) ctx.applyMark("reviewed");
        break;
      case "m":
        ctx.openMenuAtCursor();
        break;
      case "c":
        if (!e.repeat) ctx.startComment();
        break;
      case "r":
        ctx.openRevisions();
        break;
      case "x": {
        const file = lines[cursor]?.file;
        if (file !== undefined && !e.repeat) ctx.toggleChecked(file);
        break;
      }
      default:
        return;
    }
    e.preventDefault();
  };
}
