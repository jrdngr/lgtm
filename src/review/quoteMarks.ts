/** Underlines for quoted text in comments on the PR, drawn with the CSS Custom Highlight API so
 * the rendered markdown stays as it is. Each comment block sets its own ranges. */

const NAME = "lgtm-quote";
const byBlock = new Map<number, Range[]>();

function redraw() {
  if (typeof CSS === "undefined" || !("highlights" in CSS)) return;
  const all = [...byBlock.values()].flat();
  if (all.length === 0) CSS.highlights.delete(NAME);
  else CSS.highlights.set(NAME, new Highlight(...all));
}

/** The range of `text` inside `root`'s text, ignoring differences in whitespace. */
function findRange(root: Node, text: string): Range | undefined {
  const want = text.replace(/\s+/g, " ").trim();
  if (!want) return undefined;
  // Each character of the flattened text, with where it came from.
  let flat = "";
  const at: [Text, number][] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const t = n as Text;
    for (let i = 0; i < t.data.length; i++) {
      const space = /\s/.test(t.data[i]);
      if (space && flat.endsWith(" ")) continue;
      flat += space ? " " : t.data[i];
      at.push([t, i]);
    }
  }
  const start = flat.indexOf(want);
  if (start < 0) return undefined;
  const [startNode, startOffset] = at[start];
  const [endNode, endOffset] = at[start + want.length - 1];
  const range = document.createRange();
  range.setStart(startNode, startOffset);
  range.setEnd(endNode, endOffset + 1);
  return range;
}

/** Underlines `quotes` inside `root` for block `id`, replacing what it had. */
export function markQuotes(id: number, root: Node | null, quotes: string[]) {
  const ranges = root ? quotes.flatMap((q) => findRange(root, q) ?? []) : [];
  if (ranges.length) byBlock.set(id, ranges);
  else byBlock.delete(id);
  redraw();
}
