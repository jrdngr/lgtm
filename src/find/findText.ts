/** Where `query` starts in `text`, without overlaps. Case matters only with `matchCase`. */
export function findAll(text: string, query: string, matchCase: boolean): number[] {
  if (!query) return [];
  const hay = matchCase ? text : text.toLowerCase();
  const needle = matchCase ? query : query.toLowerCase();
  const out: number[] = [];
  for (let i = hay.indexOf(needle); i >= 0; i = hay.indexOf(needle, i + needle.length)) {
    out.push(i);
  }
  return out;
}

/** The match after (or before) `at` among `total`, wrapping around. */
export function stepMatch(at: number, total: number, back: boolean): number {
  if (total === 0) return 0;
  return (at + (back ? -1 : 1) + total) % total;
}
