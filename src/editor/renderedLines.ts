/** The source lines of the rendered blocks the selection touches in `root`, or of the block
 * at `at` when nothing is selected there. Blocks carry their lines as `data-start` and
 * `data-end`. */
export function renderedLines(root: HTMLElement, at?: Node): [number, number] | undefined {
  const sel = root.ownerDocument.getSelection();
  const nodes =
    sel && !sel.isCollapsed && root.contains(sel.anchorNode) && root.contains(sel.focusNode)
      ? [sel.anchorNode, sel.focusNode]
      : at && root.contains(at)
        ? [at]
        : [];
  const blocks = nodes.flatMap((n) => {
    const el = n instanceof Element ? n : n?.parentElement;
    const block = el?.closest<HTMLElement>("[data-start]");
    return block && root.contains(block) ? [block] : [];
  });
  if (!blocks.length) return undefined;
  const starts = blocks.map((b) => Number(b.dataset.start));
  const ends = blocks.map((b) => Number(b.dataset.end));
  return [Math.min(...starts), Math.max(...ends)];
}

/** `path:line`, or `path:start-end` over several lines. */
export const lineRef = (path: string, [a, b]: [number, number]) =>
  `${path}:${a === b ? a : `${a}-${b}`}`;
