import { findAll } from "./findText";

const ALL = "lgtm-find";
const CURRENT = "lgtm-find-current";

const supported = () => typeof CSS !== "undefined" && "highlights" in CSS;

/** Ranges over each match of `query` in `root`'s text, which elements may split up. */
export function textRanges(root: Node, query: string, matchCase: boolean): Range[] {
  if (!query) return [];
  const nodes: Text[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) nodes.push(n as Text);
  const starts: number[] = [];
  let pos = 0;
  for (const n of nodes) {
    starts.push(pos);
    pos += n.data.length;
  }
  const text = nodes.map((n) => n.data).join("");
  /** The node and offset of `offset`, the end of a node rather than the next one's start
   * when `end`. */
  const at = (offset: number, end: boolean): [Text, number] => {
    let i = nodes.length - 1;
    while (i > 0 && (starts[i] > offset || (end && starts[i] === offset))) i--;
    return [nodes[i], offset - starts[i]];
  };
  return findAll(text, query, matchCase).map((start) => {
    const range = document.createRange();
    range.setStart(...at(start, false));
    range.setEnd(...at(start + query.length, true));
    return range;
  });
}

/** Highlights `ranges`, and `current` among them more strongly. */
export function paint(ranges: Range[], current?: Range) {
  if (!supported()) return;
  CSS.highlights.set(ALL, new Highlight(...ranges));
  if (current) CSS.highlights.set(CURRENT, new Highlight(current));
  else CSS.highlights.delete(CURRENT);
}

export function unpaint() {
  if (!supported()) return;
  CSS.highlights.delete(ALL);
  CSS.highlights.delete(CURRENT);
}

/** Scrolls `range` into the middle of its scroller unless it's in view. */
export function reveal(range: Range) {
  const el = range.startContainer.parentElement;
  el?.scrollIntoView({ block: "nearest", inline: "nearest" });
  const box = range.getBoundingClientRect();
  const scroller = el?.closest("[data-find-scroll]") ?? null;
  if (!scroller) return;
  const view = scroller.getBoundingClientRect();
  if (box.top < view.top || box.bottom > view.bottom) {
    scroller.scrollTop += box.top - view.top - view.height / 2;
  }
}
