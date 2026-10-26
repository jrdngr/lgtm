import type { IDisposable, ILink, Terminal } from "@xterm/xterm";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { heldLink } from "./termLinks";

/** A path in a line of terminal output, by character, with the line it names, if any. */
export interface PathAt {
  from: number;
  to: number;
  path: string;
  line?: number;
}

/** A path-like word not inside another, then `:12`, `:12:3` or `#L12`. */
const PATH =
  /(?<![\w/.:@+~-])((?:~|\.{1,2})?\/?[\w@+-][\w@.+-]*(?:\/[\w@.+-]+)*)(?::(\d+)(?::\d+)?|#L(\d+))?/g;
/** "line 35 of " or "lines 35-40 in " just before a path. */
const LINE_BEFORE = /\blines? (\d+)(?:\s*[-–]\s*\d+)? (?:of|in) $/i;

/** The words in `text` that could be file paths: with a folder or an extension. Trailing
 * dots end the sentence, not the path. */
export function pathsIn(text: string): PathAt[] {
  const out: PathAt[] = [];
  for (const m of text.matchAll(PATH)) {
    const path = m[1].replace(/\.+$/, "");
    if (!path.includes("/") && !/\.[A-Za-z0-9]+$/.test(path)) continue;
    const suffix = path.length === m[1].length ? m[0].length - m[1].length : 0;
    const said = m[2] ?? m[3] ?? text.slice(0, m.index).match(LINE_BEFORE)?.[1];
    out.push({
      from: m.index,
      to: m.index + path.length + suffix,
      path,
      line: said === undefined ? undefined : Number(said),
    });
  }
  return out;
}

/** ⌘-clicking a workspace file's path in the terminal's output opens it, at its line. */
export function fileLinks(term: Terminal, workspace: WorkspaceName): IDisposable {
  return term.registerLinkProvider({
    async provideLinks(y, callback) {
      const text = term.buffer.active.getLine(y - 1)?.translateToString(true) ?? "";
      const found = pathsIn(text);
      if (!found.length) return callback(undefined);
      const r = await commands.filesAt(
        workspace,
        found.map((f) => f.path),
      );
      if (r.status !== "ok") return callback(undefined);
      const links: ILink[] = found.flatMap((f, i) => {
        const path = r.data[i];
        if (!path) return [];
        const range = { start: { x: f.from + 1, y }, end: { x: f.to, y } };
        return [heldLink(range, text.slice(f.from, f.to), () => openFile(workspace, path, f.line))];
      });
      callback(links);
    },
  });
}
