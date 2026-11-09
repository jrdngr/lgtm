import { type RefObject, useEffect, useRef, useState } from "react";
import type { Buffer } from "../editor/modal/buffer";
import { Keys } from "../editor/modal/keys";
import { copy as copyText } from "../ui/contextMenu";
import type { DisplayLine, FlatFile, ViewMode } from "./model";
import { lineEdge, lineTexts, selectedOn, selectedText, startBuffer } from "./textSelect";

const SEL = "lgtm-text-sel";
const HEAD = "lgtm-text-head";

interface Deps {
  files: FlatFile[];
  lines: DisplayLine[];
  mode: ViewMode;
  scrollRef: RefObject<HTMLElement | null>;
  /** Puts the review's cursor on a line. */
  place(line: number): void;
  pageLines(): number;
  /** Opens find with `query`. */
  find(query: string): void;
  /** What find looks for, while it's open, for `n` and `N`. */
  findQuery(): string | undefined;
}

interface Selecting {
  buf: Buffer;
  keys: Keys;
  text: string;
}

/** The chars `from..to` of the text under `el`, as a DOM range. */
function charRange(el: Element, from: number, to: number): Range | undefined {
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
  const range = document.createRange();
  let pos = 0;
  let started = false;
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const len = n.textContent?.length ?? 0;
    if (!started && from <= pos + len) {
      range.setStart(n, from - pos);
      started = true;
    }
    if (started && to <= pos + len) {
      range.setEnd(n, to - pos);
      return range;
    }
    pos += len;
  }
  return undefined;
}

/** The `.code` a line shows its text in: the new side's in split view. */
function codeOf(el: Element, line: DisplayLine, mode: ViewMode): Element | null {
  if (mode !== "split") return el.querySelector(".code");
  const right = line.kind === "context" || (line.kind === "rows" && line.right !== undefined);
  return el.querySelector(`.cell.${right ? "right" : "left"} .code`);
}

/** `v` text selection over the review: the editor's keys, read-only, over its code. */
export function useTextSelect(d: Deps) {
  const [sel, setSel] = useState<Selecting>();
  const [, redraw] = useState(0);
  const deps = useRef(d);
  deps.current = d;

  // Ends when the lines no longer read the same.
  const { files, lines, mode, scrollRef } = d;
  useEffect(() => {
    if (sel && lineTexts(files, lines, mode) !== sel.text) setSel(undefined);
  }, [files, lines, mode, sel]);

  /** The line whose cursor sits past its text, which the line draws itself. */
  let eolLine: number | undefined;
  if (sel) {
    const b = sel.buf;
    const line = b.lineOf(b.sel.head);
    if (b.sel.head >= b.lineEnd(line)) eolLine = line;
  }

  // Draws the selection over the code lines rendered now, and again as others render.
  useEffect(() => {
    const root = scrollRef.current;
    if (!sel || !root || typeof CSS === "undefined" || !("highlights" in CSS)) return;
    let frame = 0;
    const draw = () => {
      frame = 0;
      const ranges: Range[] = [];
      let cursor: Range | undefined;
      for (const el of root.querySelectorAll<HTMLElement>(".line[data-index]")) {
        const i = Number(el.dataset.index);
        const on = selectedOn(sel.buf, i);
        const code = on && lines[i] && codeOf(el, lines[i], mode);
        if (!on || !code) continue;
        const r = on.from < on.to ? charRange(code, on.from, on.to) : undefined;
        if (r) ranges.push(r);
        if (on.head !== undefined) cursor = charRange(code, on.head, on.head + 1) ?? cursor;
      }
      CSS.highlights.set(SEL, new Highlight(...ranges));
      CSS.highlights.set(HEAD, new Highlight(...(cursor ? [cursor] : [])));
      // Brings the cursor's char into view sideways.
      const box = cursor?.getBoundingClientRect();
      const view = root.getBoundingClientRect();
      if (box && box.width > 0) {
        if (box.right > view.right) root.scrollBy({ left: box.right - view.right + 32 });
        else if (box.left < view.left + 120) root.scrollBy({ left: box.left - view.left - 120 });
      }
    };
    draw();
    const changes = new MutationObserver(() => {
      if (!frame) frame = requestAnimationFrame(draw);
    });
    changes.observe(root, { childList: true, subtree: true });
    return () => {
      changes.disconnect();
      cancelAnimationFrame(frame);
      CSS.highlights.delete(SEL);
      CSS.highlights.delete(HEAD);
    };
  });

  return {
    on: !!sel,
    /** The line whose cursor is past its text. */
    eolLine,
    /** Starts selecting on `line`. */
    start(line: number) {
      const { files, lines, mode } = deps.current;
      const text = lineTexts(files, lines, mode);
      setSel({ buf: startBuffer(text, line), keys: new Keys(true, "select"), text });
    },
    stop() {
      setSel(undefined);
    },
    /** Handles a key while selecting: motions move the selection, Home and End go to the
     * line's ends, `y` copies it, `/` finds from it, and `v` or Esc stop. */
    key(e: React.KeyboardEvent, mod: boolean) {
      if (!sel) return;
      const { buf: b, keys } = sel;
      const d = deps.current;
      const waiting = keys.waiting();
      let end = false;
      if (!waiting && (e.key === "Home" || e.key === "End")) lineEdge(b, e.key === "End");
      else if ((!waiting && e.key === "y" && !mod) || (mod && e.key.toLowerCase() === "c")) {
        copyText(selectedText(b));
        end = true;
      } else if (mod) return;
      else {
        if (!waiting && (e.key === "n" || e.key === "N")) {
          const q = d.findQuery();
          if (q) keys.search = q;
        }
        const action = keys.press(
          b,
          { key: e.key, ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey },
          { height: d.pageLines(), hidden: [] },
        );
        if (action.a === "copy") copyText(action.text);
        if (action.a === "search") {
          const picked = selectedText(b);
          d.find(picked.includes("\n") ? "" : picked);
        }
        end = action.a === "copy" || keys.mode !== "select";
      }
      e.preventDefault();
      if (end) setSel(undefined);
      else {
        d.place(b.lineOf(b.sel.head));
        redraw((n) => n + 1);
      }
    },
  };
}
