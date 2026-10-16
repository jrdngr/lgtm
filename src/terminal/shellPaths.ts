/** Paths as a shell reads them: quoted where needed, space-separated, with a trailing space
 * so the next word can follow, as Terminal does for dropped files. */
export function shellPaths(paths: string[]): string {
  return paths.map(quote).join(" ") + (paths.length ? " " : "");
}

function quote(path: string): string {
  // Newlines would run the command; no real path needs them.
  const p = path.replace(/[\r\n]/g, "");
  return /^[\w@%+=:,./-]+$/.test(p) ? p : `'${p.replace(/'/g, `'\\''`)}'`;
}
