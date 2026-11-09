import { useRef } from "react";
import type { MarkColor } from "../bindings";
import { isMac } from "../core/keys";
import { type MenuAction, menuAction } from "./MarkMenu";
import { colorForKey, type MarkSetup } from "./marks";
import { type DisplayLine, type FlatFile, nextRowLine } from "./model";
import { expand, reviewSelection } from "./selection";
import type { SelectionApi } from "./useSelection";
import type { useTextSelect } from "./useTextSelect";

interface Ctx {
  files: FlatFile[];
  lines: DisplayLine[];
  s: SelectionApi;
  setup: MarkSetup;
  menu: boolean;
  closeMenu(): void;
  pick(action: MenuAction): void;
  step(undo: boolean): void;
  copy(): void;
  /** Sends where the selection is to the most recently used terminal. */
  sendToTerminal(): void;
  applyMark(color: MarkColor | null): void;
  /** Whether `space` waits for its next key. */
  space: boolean;
  setSpace(on: boolean): void;
  /** Runs the space menu's `key`. */
  spaceKey(key: string): void;
  /** `v` text selection. */
  text: ReturnType<typeof useTextSelect>;
  /** How many lines the review shows at once. */
  pageLines(): number;
  scrollSide(dir: 1 | -1): void;
  stepFile(dir: 1 | -1): void;
  stepItem(dir: 1 | -1): void;
  stepComment(dir: 1 | -1): void;
  toggleFile(file: number): void;
  toggleChecked(file: number): void;
  markFile(file: number, color: MarkColor): void;
  markHunk(file: number, hunk: number, color: MarkColor): void;
  /** Opens a comment for the agent on the selection. */
  startComment(): void;
  /** Edits the comment under the cursor's line. */
  editComment(): void;
  deleteComment(): void;
  toggleResolved(): void;
  find(): void;
  /** Steps through the find's matches while it's open. Returns whether it did. */
  findStep(back: boolean): boolean;
  closeFind(): boolean;
  toggleHideReviewed(): void;
  toggleHidePr(): void;
  pull(): void;
  toggleSplit(): void;
}

/** Form fields keep their own keys. */
const isEditable = (t: EventTarget) =>
  t instanceof HTMLInputElement ||
  t instanceof HTMLSelectElement ||
  t instanceof HTMLTextAreaElement;

/** The review's keyboard handler: the TUI review's keys. */
export function useReviewKeys(ctx: Ctx): (e: React.KeyboardEvent) => void {
  const pendingZ = useRef(false);
  const { files, lines, s } = ctx;
  const { cursor, select } = s;

  /** Puts the cursor `by` lines away, on the nearest row line. */
  function moveBy(by: number) {
    const to = Math.max(0, Math.min(lines.length - 1, cursor + by));
    if (lines[to]?.kind === "rows") return s.place(to);
    const dir = by > 0 ? 1 : -1;
    const near = nextRowLine(lines, to, dir);
    s.place(near !== to ? near : nextRowLine(lines, to, dir === 1 ? -1 : 1));
  }

  return (e) => {
    if (isEditable(e.target)) return;
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (ctx.text.on) return ctx.text.key(e, mod);
    if (ctx.menu) {
      // Selected code text copies natively, as it does with the menu closed.
      if (mod && e.key.toLowerCase() === "c" && window.getSelection()?.isCollapsed === false) {
        ctx.closeMenu();
        return;
      }
      e.preventDefault();
      const z = pendingZ.current;
      pendingZ.current = false;
      if (e.key === "Escape") ctx.closeMenu();
      else if (mod) {
        if (e.key.toLowerCase() === "c") ctx.pick("copy");
      } else if (e.key === "z") pendingZ.current = true;
      else if (z && e.key === "a") ctx.pick("collapse");
      else {
        const action = menuAction(e.key, ctx.setup);
        if (action && !e.repeat) ctx.pick(action);
      }
      return;
    }
    if (ctx.space) {
      e.preventDefault();
      if (["Shift", "Alt", "Control", "Meta"].includes(e.key)) return;
      ctx.setSpace(false);
      ctx.spaceKey(e.key);
      return;
    }
    if (mod) {
      const key = e.key.toLowerCase();
      if (key === "a") select(reviewSelection(lines));
      else if (e.key === ">" || e.key === ".") {
        if (!e.shiftKey) return;
        ctx.sendToTerminal();
      } else if (key === "z") ctx.step(!e.shiftKey);
      else if (key === "c") {
        // Selected code text copies natively; otherwise copy the selected rows.
        if (window.getSelection()?.isCollapsed === false) return;
        ctx.copy();
      } else return;
      e.preventDefault();
      return;
    }
    if (e.ctrlKey) {
      if (e.key === "d") moveBy(Math.floor(ctx.pageLines() / 2));
      else if (e.key === "u") moveBy(-Math.floor(ctx.pageLines() / 2));
      else return;
      e.preventDefault();
      return;
    }
    if (e.altKey) return;
    const line = lines[cursor];
    const once = !e.repeat;
    switch (e.key) {
      case " ":
        ctx.setSpace(true);
        break;
      case "j":
      case "ArrowDown":
        s.move(1, false);
        break;
      case "k":
      case "ArrowUp":
        s.move(-1, false);
        break;
      case "J":
      case "K":
        s.extend(e.key === "J" ? 1 : -1);
        break;
      case "g":
      case "Home":
        moveBy(-lines.length);
        break;
      case "G":
      case "End":
        moveBy(lines.length);
        break;
      case "PageDown":
        moveBy(ctx.pageLines());
        break;
      case "PageUp":
        moveBy(-ctx.pageLines());
        break;
      case "h":
      case "ArrowLeft":
        ctx.scrollSide(-1);
        break;
      case "l":
      case "ArrowRight":
        ctx.scrollSide(1);
        break;
      case "v":
        ctx.text.start(cursor);
        break;
      case "e": {
        const next = expand(files, lines, cursor, s.sel);
        if (next) s.grow(next);
        break;
      }
      case "E":
        s.shrink();
        break;
      case "Escape":
        if (!ctx.closeFind()) s.collapse();
        break;
      case "1":
      case "2":
      case "3":
      case "4":
      case "5":
      case "6":
      case "7":
      case "8":
      case "9": {
        const color = colorForKey(ctx.setup, e.key);
        if (color && once) ctx.applyMark(color);
        break;
      }
      case "0":
        if (ctx.setup.enabled && once) ctx.applyMark(null);
        break;
      case "n":
      case "N":
        if (!ctx.findStep(e.key === "N")) ctx.stepFile(e.key === "n" ? 1 : -1);
        break;
      case "m":
      case "M":
        ctx.stepItem(e.key === "m" ? 1 : -1);
        break;
      case "]":
      case "[":
        ctx.stepComment(e.key === "]" ? 1 : -1);
        break;
      case "r":
        if (line && once) ctx.toggleChecked(line.file);
        break;
      case "f":
        if (line && once) ctx.markFile(line.file, "red");
        break;
      case "R":
      case "F":
        if (line && line.kind !== "file" && line.hunk >= 0 && once)
          ctx.markHunk(line.file, line.hunk, e.key === "R" ? "green" : "red");
        break;
      case "u":
      case "U":
        ctx.step(e.key === "u");
        break;
      case "c":
        if (once) ctx.startComment();
        break;
      case "Enter":
        if (line?.kind === "file") ctx.toggleFile(line.file);
        else ctx.editComment();
        break;
      case "d":
        if (once) ctx.deleteComment();
        break;
      case "C":
        if (once) ctx.toggleResolved();
        break;
      case "/":
        ctx.find();
        break;
      case "z":
        ctx.toggleHideReviewed();
        break;
      case "P":
        ctx.toggleHidePr();
        break;
      case "p":
        if (once) ctx.pull();
        break;
      case "t":
        ctx.toggleSplit();
        break;
      default:
        return;
    }
    e.preventDefault();
  };
}
