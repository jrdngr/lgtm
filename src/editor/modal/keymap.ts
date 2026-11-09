/** The one key table: which keys run which command in which modes. Mirrors
 * `crates/lgtm/src/edit/keymap.rs` line for line. Keys are names joined by spaces, like
 * `g g`, `space f` or `C-d`. */

export type Fold =
  | "toggle"
  | "close"
  | "open"
  | "closeAll"
  | "openAll"
  | "closeLevel"
  | "openLevel";

/** The lists `space` opens to pick from. */
export type Pick = "files" | "openFiles" | "grep";

export type Cmd =
  | { c: "left" | "right" | "down" | "up" | "halfDown" | "halfUp" }
  | { c: "nextWord" | "wordEnd" | "prevWord"; big: boolean }
  | { c: "find"; forward: boolean; till: boolean }
  | { c: "fileStart" | "fileEnd" | "lineStart" | "lineEnd" | "lineText" }
  | { c: "selectLine" | "toggleSelect" | "collapse" | "selectAll" }
  | { c: "insert" | "append" | "insertLineStart" | "appendLineEnd" | "openBelow" | "openAbove" }
  | { c: "delete" | "change" | "yank" | "pasteAfter" | "pasteBefore" | "replace" }
  | { c: "indent" | "outdent" | "join" | "toggleCase" }
  | { c: "object"; around: boolean }
  | { c: "grow" | "shrink" }
  | { c: "fold"; fold: Fold }
  | { c: "surroundAdd" | "surroundDelete" | "surroundReplace" }
  | { c: "undo" | "redo" | "search" | "searchNext" | "searchPrev" | "searchSelection" }
  | { c: "clipboardYank" | "clipboardPaste" }
  | { c: "pick"; pick: Pick }
  | { c: "review" | "sidebar" | "toggleSidebar" }
  | { c: "split"; right: boolean }
  | { c: "command" | "escape" | "smart" | "toggleCheck" };

const EDITS = new Set([
  "insert",
  "append",
  "insertLineStart",
  "appendLineEnd",
  "openBelow",
  "openAbove",
  "delete",
  "change",
  "pasteAfter",
  "pasteBefore",
  "replace",
  "indent",
  "outdent",
  "join",
  "toggleCase",
  "surroundAdd",
  "surroundDelete",
  "surroundReplace",
  "undo",
  "redo",
  "clipboardPaste",
  "smart",
  "toggleCheck",
]);

/** Whether it changes the text. */
export const edits = (cmd: Cmd) => EDITS.has(cmd.c);

/** The modes a binding works in. */
type In = "select" | "both";

export interface Bind {
  keys: string;
  cmd: Cmd;
  modes: In;
  /** What it does, for the prefix menus. */
  label: string;
}

const b = (keys: string, cmd: Cmd, modes: In, label: string): Bind => ({
  keys,
  cmd,
  modes,
  label,
});

const Both = "both";

export const BINDS: Bind[] = [
  b("h", { c: "left" }, Both, "left"),
  b("left", { c: "left" }, Both, "left"),
  b("l", { c: "right" }, Both, "right"),
  b("right", { c: "right" }, Both, "right"),
  b("j", { c: "down" }, Both, "down"),
  b("down", { c: "down" }, Both, "down"),
  b("k", { c: "up" }, Both, "up"),
  b("up", { c: "up" }, Both, "up"),
  b("C-d", { c: "halfDown" }, Both, "half page down"),
  b("C-u", { c: "halfUp" }, Both, "half page up"),
  b("w", { c: "nextWord", big: false }, Both, "word"),
  b("W", { c: "nextWord", big: true }, Both, "WORD"),
  b("e", { c: "wordEnd", big: false }, Both, "word end"),
  b("E", { c: "wordEnd", big: true }, Both, "WORD end"),
  b("b", { c: "prevWord", big: false }, Both, "word back"),
  b("B", { c: "prevWord", big: true }, Both, "WORD back"),
  b("f", { c: "find", forward: true, till: false }, Both, "find char"),
  b("t", { c: "find", forward: true, till: true }, Both, "till char"),
  b("F", { c: "find", forward: false, till: false }, Both, "find char back"),
  b("T", { c: "find", forward: false, till: true }, Both, "till char back"),
  b("g g", { c: "fileStart" }, Both, "file start"),
  b("g e", { c: "fileEnd" }, Both, "file end"),
  b("g h", { c: "lineStart" }, Both, "line start"),
  b("home", { c: "lineStart" }, Both, "line start"),
  b("g l", { c: "lineEnd" }, Both, "line end"),
  b("end", { c: "lineEnd" }, Both, "line end"),
  b("g s", { c: "lineText" }, Both, "first non-blank"),
  b("x", { c: "selectLine" }, Both, "select line"),
  b("v", { c: "toggleSelect" }, Both, "select mode"),
  b(";", { c: "collapse" }, Both, "collapse selection"),
  b("%", { c: "selectAll" }, Both, "select all"),
  b("i", { c: "insert" }, Both, "insert"),
  b("a", { c: "append" }, Both, "append"),
  b("I", { c: "insertLineStart" }, Both, "insert at line start"),
  b("A", { c: "appendLineEnd" }, Both, "append at line end"),
  b("o", { c: "openBelow" }, Both, "line below"),
  b("O", { c: "openAbove" }, Both, "line above"),
  b("d", { c: "delete" }, Both, "delete"),
  b("c", { c: "change" }, Both, "change"),
  b("y", { c: "yank" }, Both, "yank"),
  b("p", { c: "pasteAfter" }, Both, "paste after"),
  b("P", { c: "pasteBefore" }, Both, "paste before"),
  b("r", { c: "replace" }, Both, "replace with a char"),
  b(">", { c: "indent" }, Both, "indent"),
  b("<", { c: "outdent" }, Both, "outdent"),
  b("J", { c: "join" }, Both, "join lines"),
  b("~", { c: "toggleCase" }, Both, "toggle case"),
  b("m i", { c: "object", around: false }, Both, "inside an object"),
  b("m a", { c: "object", around: true }, Both, "around an object"),
  b("A-o", { c: "grow" }, Both, "grow to the syntax node"),
  b("A-i", { c: "shrink" }, Both, "shrink to a syntax node"),
  b("m s", { c: "surroundAdd" }, Both, "add surround"),
  b("m d", { c: "surroundDelete" }, Both, "delete surround"),
  b("m r", { c: "surroundReplace" }, Both, "replace surround"),
  b("u", { c: "undo" }, Both, "undo"),
  b("U", { c: "redo" }, Both, "redo"),
  b("/", { c: "search" }, Both, "search"),
  b("n", { c: "searchNext" }, Both, "next match"),
  b("N", { c: "searchPrev" }, Both, "previous match"),
  b("*", { c: "searchSelection" }, Both, "search the selection"),
  b("z z", { c: "fold", fold: "toggle" }, Both, "toggle"),
  b("z c", { c: "fold", fold: "close" }, Both, "close"),
  b("z o", { c: "fold", fold: "open" }, Both, "open"),
  b("z M", { c: "fold", fold: "closeAll" }, Both, "close all"),
  b("z R", { c: "fold", fold: "openAll" }, Both, "open all"),
  b("z m", { c: "fold", fold: "closeLevel" }, Both, "close a level"),
  b("z r", { c: "fold", fold: "openLevel" }, Both, "open a level"),
  b("space f", { c: "pick", pick: "files" }, Both, "files"),
  b("space o", { c: "pick", pick: "openFiles" }, Both, "open files"),
  b("space /", { c: "pick", pick: "grep" }, Both, "search"),
  b("space e", { c: "sidebar" }, Both, "file tree"),
  b("space b", { c: "toggleSidebar" }, Both, "toggle file tree"),
  b("space r", { c: "review" }, Both, "review"),
  b("space x", { c: "toggleCheck" }, Both, "toggle checkbox"),
  b("S-tab", { c: "smart" }, Both, "markdown table cell, footnote"),
  b("C-w v", { c: "split", right: true }, Both, "split to the right"),
  b("C-w s", { c: "split", right: false }, Both, "split below"),
  b("space y", { c: "clipboardYank" }, Both, "yank to clipboard"),
  b("space p", { c: "clipboardPaste" }, Both, "paste from clipboard"),
  b(":", { c: "command" }, Both, "command"),
  b("esc", { c: "escape" }, "select", "normal mode"),
];

/** A key as the keys see it: a DOM `KeyboardEvent.key` and its modifiers. */
export interface Key {
  key: string;
  ctrl?: boolean;
  alt?: boolean;
  shift?: boolean;
}

const NAMED: Record<string, string> = {
  " ": "space",
  Escape: "esc",
  Enter: "ret",
  Backspace: "bs",
  Delete: "del",
  Tab: "tab",
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
  Home: "home",
  End: "end",
  PageUp: "pgup",
  PageDown: "pgdn",
};

/** The name `key` has in the table. */
export function name(k: Key): string | undefined {
  const base = NAMED[k.key] ?? (k.key.length === 1 ? k.key : undefined);
  if (base === undefined) return undefined;
  if (base === "tab" && k.shift) return "S-tab";
  return `${k.ctrl ? "C-" : ""}${k.alt ? "A-" : ""}${base}`;
}

const fits = (bind: Bind, select: boolean) => bind.modes === "both" || select;

export type Found = { run: Cmd } | "partial" | "none";

/** What the keys typed so far come to in `select` mode or not. */
export function find(seq: string, select: boolean): Found {
  let partial = false;
  for (const bind of BINDS) {
    if (!fits(bind, select)) continue;
    if (bind.keys === seq) return { run: bind.cmd };
    if (bind.keys.startsWith(`${seq} `)) partial = true;
  }
  return partial ? "partial" : "none";
}

/** The bindings after `prefix`, each with its next key and label, for a prefix's menu. */
export function after(prefix: string, select: boolean): [string, string][] {
  const start = `${prefix} `;
  return BINDS.filter((x) => fits(x, select) && x.keys.startsWith(start)).map((x) => [
    x.keys.slice(start.length),
    x.label,
  ]);
}
