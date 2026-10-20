/** GitHub's id for a heading: lower case, punctuation dropped, spaces to dashes. */
export function slug(heading: string): string {
  return heading
    .trim()
    .toLowerCase()
    .replace(/<[^>]*>/g, "")
    .replace(/[^\p{L}\p{N}\p{M}\s_-]/gu, "")
    .replace(/\s/g, "-");
}

/** The heading text of an ATX (`## A`) or setext (`A` over `===`) heading line, without
 * inline markup that doesn't reach the id. */
function headingText(line: string): string {
  return line
    .replace(/^ {0,3}#{1,6}\s+/, "")
    .replace(/\s+#+\s*$/, "")
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/[*_`~]/g, "");
}

/** The 1-based line `anchor` points at in markdown `text`: a GitHub line link (`L12`,
 * `L12-L20`), a heading's id (with `-1`, `-2` for repeats), or an element's `id` or `name`.
 * Headings inside fenced code don't count. */
export function anchorLine(text: string, anchor: string): number | undefined {
  const lineLink = /^L(\d+)(?:-L\d+)?$/.exec(anchor);
  if (lineLink) return Number(lineLink[1]);
  const want = anchor.toLowerCase();
  const lines = text.split("\n");
  const seen = new Map<string, number>();
  let fence: string | undefined;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const f = /^ {0,3}(`{3,}|~{3,})/.exec(line)?.[1];
    if (f && (!fence || f[0] === fence[0])) {
      fence = fence ? undefined : f;
      continue;
    }
    if (fence) continue;
    const ids = [...line.matchAll(/\b(?:id|name)=["']([^"']+)["']/g)].map((m) => m[1]);
    if (ids.some((id) => id.toLowerCase() === want)) return i + 1;
    const next = lines[i + 1] ?? "";
    const atx = /^ {0,3}#{1,6}(\s|$)/.test(line);
    const setext = !atx && line.trim() !== "" && /^ {0,3}(=+|-+)\s*$/.test(next);
    if (!atx && !setext) continue;
    const base = slug(headingText(line));
    const n = seen.get(base) ?? 0;
    seen.set(base, n + 1);
    if ((n ? `${base}-${n}` : base) === want) return i + 1;
  }
  return undefined;
}
