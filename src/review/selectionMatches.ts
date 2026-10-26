import { type RefObject, useEffect, useState } from "react";

const NAME = "lgtm-match";
const WORD = /^[\w$]+$/;
const WORD_CHAR = /[\w$]/;

/** What to look for when `selected` is selected: single-line text of 2 or more characters. */
export function needleOf(selected: string): string | undefined {
  return selected.length >= 2 && !/[\r\n]/.test(selected) ? selected : undefined;
}

/** Where `needle` starts in `text`, case-sensitively. A word only matches whole words. */
export function matchOffsets(text: string, needle: string): number[] {
  const word = WORD.test(needle);
  const out: number[] = [];
  for (let i = text.indexOf(needle); i >= 0; i = text.indexOf(needle, i + needle.length)) {
    const end = i + needle.length;
    if (word && (WORD_CHAR.test(text[i - 1] ?? "") || WORD_CHAR.test(text[end] ?? ""))) continue;
    out.push(i);
  }
  return out;
}

/** The `.code` element holding `node`. */
const codeOf = (node: Node | null) =>
  (node instanceof Element ? node : node?.parentElement)?.closest(".code") ?? null;

/** The selected code text inside `root`, when the selection stays in one line's code. */
function selectedNeedle(root: HTMLElement): string | undefined {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed) return undefined;
  const code = codeOf(sel.anchorNode);
  if (!code || code !== codeOf(sel.focusNode) || !root.contains(code)) return undefined;
  return needleOf(sel.toString());
}

/** Ranges over each match of `needle` in `code`'s text, which its highlight spans split up. */
function rangesIn(code: Element, needle: string): Range[] {
  const nodes: Text[] = [];
  const walker = document.createTreeWalker(code, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) nodes.push(n as Text);
  const text = nodes.map((n) => n.data).join("");
  const at = (offset: number, end: boolean): [Text, number] => {
    let pos = 0;
    for (const n of nodes) {
      const next = pos + n.data.length;
      if (offset < next || (end && offset === next)) return [n, offset - pos];
      pos = next;
    }
    const last = nodes[nodes.length - 1];
    return [last, last.data.length];
  };
  return matchOffsets(text, needle).map((start) => {
    const range = document.createRange();
    range.setStart(...at(start, false));
    range.setEnd(...at(start + needle.length, true));
    return range;
  });
}

/** Highlights every match of the selected code text across the code rendered in `rootRef`,
 * redrawing as rows scroll in. Returns the text it looks for. */
export function useSelectionMatches(rootRef: RefObject<HTMLElement | null>): string | undefined {
  const [shown, setShown] = useState<string>();
  useEffect(() => {
    const root = rootRef.current;
    if (!root || typeof CSS === "undefined" || !("highlights" in CSS)) return;
    let needle: string | undefined;
    let frame = 0;
    const draw = () => {
      frame = 0;
      if (!needle) {
        CSS.highlights.delete(NAME);
        return;
      }
      const want = needle;
      const ranges = [...root.querySelectorAll(".code")].flatMap((c) => rangesIn(c, want));
      CSS.highlights.set(NAME, new Highlight(...ranges));
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(draw);
    };
    const onSelection = () => {
      const next = selectedNeedle(root);
      if (next === needle) return;
      needle = next;
      setShown(next);
      schedule();
    };
    const rows = new MutationObserver(() => needle && schedule());
    rows.observe(root, { childList: true, subtree: true, characterData: true });
    document.addEventListener("selectionchange", onSelection);
    return () => {
      rows.disconnect();
      document.removeEventListener("selectionchange", onSelection);
      cancelAnimationFrame(frame);
      CSS.highlights.delete(NAME);
    };
  }, [rootRef]);
  return shown;
}
