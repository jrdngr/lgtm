/** Modes, and turning keys into commands on a buffer through the key table. Mirrors
 * `crates/lgtm/src/edit/keys.rs` and `insert.rs`. */

import { isMarkdown, type MdEdit, mdEnter, mdRenumber, mdSmart, mdToggleCheck } from "../mdEdit";
import { type Buffer, Sel } from "./buffer";
import { type Cmd, edits, type Fold, find, type Key, name, type Pick } from "./keymap";
import * as motion from "./motion";
import { object } from "./objects";
import * as ops from "./ops";

export type Mode = "normal" | "insert" | "select";

/** What a key asks of what's around the buffer. */
export type Action =
  | { a: "none" }
  /** Open the `:` command line. */
  | { a: "command" }
  /** Open the search line. */
  | { a: "search" }
  /** Put this on the system clipboard. */
  | { a: "copy"; text: string }
  /** Paste the system clipboard after the selection, through `pasteText`. */
  | { a: "paste" }
  /** Something the buffer couldn't do, to show. */
  | { a: "notice"; text: string }
  | { a: "fold"; fold: Fold }
  | { a: "pick"; pick: Pick }
  | { a: "review" }
  | { a: "sidebar" }
  | { a: "toggleSidebar" }
  | { a: "split"; right: boolean }
  /** Find a syntax node for the selection, then hand it to `syntaxFound`. */
  | { a: "syntax"; op: SyntaxOp };

/** What the syntax tree is asked for: the node to grow to, one to shrink to, or the
 * function around the selection, inside or around it. */
export type SyntaxOp = "grow" | "shrink" | "inFunction" | "aroundFunction";

const NONE: Action = { a: "none" };

/** How the screen around the buffer moves keys: lines in view, and the closed folds `j` and
 * `k` step over. */
export interface View {
  height: number;
  hidden: [number, number][];
}

/** A command waiting for the char typed after it. */
type Await =
  | { w: "find"; forward: boolean; till: boolean }
  | { w: "replace" | "surroundAdd" | "surroundDelete" }
  | { w: "surroundReplace"; from?: string }
  | { w: "object"; around: boolean };

export class Keys {
  mode: Mode = "normal";
  /** What `n` and `N` look for. */
  search: string | undefined;
  /** The keys of a binding typed so far. */
  private pendingKeys: string[] = [];
  private awaiting: Await | undefined;
  /** The buffer's version when insert mode started. */
  private insertedAt = 0;
  private register: ops.Register = { text: "", lines: false };
  /** Selections `Alt-o` grew from, for `Alt-i` to go back through. */
  private grown: Sel[] = [];

  /** `readOnly`: commands that change the text do nothing. */
  constructor(
    private readOnly = false,
    mode: Mode = "normal",
  ) {
    this.mode = mode;
  }

  /** The keys typed toward a binding, like `g` or `space`. */
  pending() {
    return this.pendingKeys.join(" ");
  }

  /** Whether a key is still to come: part of a binding, or a command's char. */
  waiting() {
    return this.pendingKeys.length > 0 || this.awaiting !== undefined;
  }

  press(b: Buffer, key: Key, view: View): Action {
    if (this.mode === "insert") {
      this.insertKey(b, key);
      return NONE;
    }
    const wait = this.awaiting;
    if (wait) {
      this.awaiting = undefined;
      return key.key.length === 1 && !key.ctrl ? this.finish(b, wait, key.key) : NONE;
    }
    const n = name(key);
    if (n === undefined) return NONE;
    this.pendingKeys.push(n);
    const found = find(this.pending(), this.mode === "select");
    if (found === "partial") return NONE;
    this.pendingKeys = [];
    if (found === "none") return NONE;
    const cmd = found.run;
    const version = b.version;
    const action = this.run(b, cmd, view);
    if (!this.inserting() && b.version !== version && cmd.c !== "undo" && cmd.c !== "redo") {
      renumber(b);
    }
    if (!["down", "up", "halfDown", "halfUp"].includes(cmd.c)) b.column = undefined;
    return action;
  }

  /** Whether typing goes into the text: a command may have started insert mode. */
  inserting() {
    return this.mode === "insert";
  }

  /** Makes `f`'s markdown edit at the cursor, as one undo step, in a markdown file. */
  markdown(b: Buffer, f: (text: string, at: number) => MdEdit | undefined) {
    if (!isMarkdown(b.name)) return false;
    const edit = f(b.text, b.sel.head);
    if (!edit) return false;
    if (this.mode !== "insert") this.edit(b);
    apply(b, edit);
    return true;
  }

  private select() {
    return this.mode === "select";
  }

  /** Takes a motion's selection, or in select mode moves only its head. */
  private take(b: Buffer, [anchor, head]: [number, number]) {
    b.sel = this.select() ? new Sel(b.sel.anchor, head) : new Sel(anchor, head);
  }

  /** Moves the cursor to `to`, the selection following it in select mode. */
  private go(b: Buffer, to: number) {
    this.take(b, [to, to]);
  }

  private vertical(b: Buffer, by: number, hidden: [number, number][]) {
    const [to, column] = motion.vertical(b, b.sel.head, by, b.column, hidden);
    b.column = column;
    this.go(b, to);
  }

  /** Starts an edit: one undo step, after which select mode ends. */
  private edit(b: Buffer) {
    b.checkpoint();
    if (this.select()) this.mode = "normal";
  }

  private run(b: Buffer, cmd: Cmd, view: View): Action {
    if (this.readOnly && edits(cmd)) return NONE;
    const half = Math.max(Math.floor(view.height / 2), 1);
    const head = b.sel.head;
    const empty = b.len === 0;
    switch (cmd.c) {
      case "left":
        this.go(b, motion.left(b, head));
        break;
      case "right":
        this.go(b, motion.right(b, head));
        break;
      case "down":
        this.vertical(b, 1, view.hidden);
        break;
      case "up":
        this.vertical(b, -1, view.hidden);
        break;
      case "halfDown":
        this.vertical(b, half, view.hidden);
        break;
      case "halfUp":
        this.vertical(b, -half, view.hidden);
        break;
      case "nextWord":
        this.take(b, motion.nextWordStart(b, head, cmd.big));
        break;
      case "wordEnd":
        this.take(b, motion.nextWordEnd(b, head, cmd.big));
        break;
      case "prevWord":
        this.take(b, motion.prevWordStart(b, head, cmd.big));
        break;
      case "find":
        this.awaiting = { w: "find", forward: cmd.forward, till: cmd.till };
        break;
      case "fileStart":
        this.go(b, 0);
        break;
      case "fileEnd":
        this.go(b, b.lineStart(b.lastLine()));
        break;
      case "lineStart":
        this.go(b, b.lineStart(b.lineOf(head)));
        break;
      case "lineEnd":
        this.go(b, motion.lineMax(b, b.lineOf(head)));
        break;
      case "lineText":
        this.go(b, motion.firstNonBlank(b, b.lineOf(head)));
        break;
      case "selectLine": {
        const [from, to] = motion.lineSelect(b, b.sel.from(), b.sel.to());
        b.sel = new Sel(from, to);
        break;
      }
      case "toggleSelect":
        this.mode = this.select() ? "normal" : "select";
        break;
      case "collapse":
        b.sel = Sel.point(head);
        break;
      case "selectAll":
        b.sel = new Sel(0, b.lastChar());
        break;
      case "insert":
        this.enterInsert(b, b.sel.from());
        break;
      case "append":
        this.enterInsert(b, Math.min(b.sel.to() + 1, b.len));
        break;
      case "insertLineStart":
        this.enterInsert(b, motion.firstNonBlank(b, b.lineOf(b.sel.from())));
        break;
      case "appendLineEnd":
        this.enterInsert(b, b.lineEnd(b.lineOf(b.sel.to())));
        break;
      case "openBelow":
      case "openAbove":
        this.openLine(b, cmd.c === "openBelow");
        break;
      case "delete":
        if (empty) break;
        this.register = ops.yank(b);
        this.edit(b);
        ops.del(b);
        break;
      case "change": {
        if (!empty) this.register = ops.yank(b);
        b.checkpoint();
        const at = empty ? 0 : ops.change(b);
        b.sel = Sel.point(at);
        this.startInsert(b);
        break;
      }
      case "yank":
        if (empty) break;
        this.register = ops.yank(b);
        if (this.select()) this.mode = "normal";
        // Yanks go on the system clipboard too.
        return { a: "copy", text: this.register.text };
      case "pasteAfter":
      case "pasteBefore": {
        const r = this.register;
        this.edit(b);
        ops.paste(b, r, cmd.c === "pasteAfter");
        b.dropEmptyCheckpoint();
        break;
      }
      case "replace":
        this.awaiting = { w: "replace" };
        break;
      case "indent":
      case "outdent":
        this.edit(b);
        ops.indent(b, cmd.c === "indent");
        b.dropEmptyCheckpoint();
        break;
      case "join":
        this.edit(b);
        ops.join(b);
        b.dropEmptyCheckpoint();
        break;
      case "toggleCase":
        if (empty) break;
        this.edit(b);
        ops.toggleCase(b);
        break;
      case "object":
        this.awaiting = { w: "object", around: cmd.around };
        break;
      case "grow":
        return { a: "syntax", op: "grow" };
      case "shrink": {
        const sel = this.grown.pop();
        if (sel && sel.from() >= b.sel.from() && sel.to() <= b.sel.to()) {
          b.sel = sel;
          break;
        }
        this.grown = [];
        return { a: "syntax", op: "shrink" };
      }
      case "fold":
        return { a: "fold", fold: cmd.fold };
      case "surroundAdd":
      case "surroundDelete":
        this.awaiting = { w: cmd.c };
        break;
      case "surroundReplace":
        this.awaiting = { w: "surroundReplace" };
        break;
      case "undo":
        b.undo();
        break;
      case "redo":
        b.redo();
        break;
      case "search":
        return { a: "search" };
      case "searchNext":
      case "searchPrev":
        return this.nextMatch(b, cmd.c === "searchNext");
      case "searchSelection":
        if (!empty) this.search = b.slice(b.sel.from(), b.sel.to() + 1);
        break;
      case "clipboardYank":
        if (!empty) return { a: "copy", text: b.slice(b.sel.from(), b.sel.to() + 1) };
        break;
      case "clipboardPaste":
        return { a: "paste" };
      case "pick":
        return { a: "pick", pick: cmd.pick };
      case "review":
      case "sidebar":
      case "toggleSidebar":
      case "command":
        return { a: cmd.c };
      case "split":
        return { a: "split", right: cmd.right };
      case "escape":
        this.mode = "normal";
        break;
      case "smart":
        this.markdown(b, mdSmart);
        break;
      case "toggleCheck":
        this.markdown(b, mdToggleCheck);
        break;
    }
    return NONE;
  }

  /** Runs a command that waited for `c`. */
  private finish(b: Buffer, wait: Await, c: string): Action {
    switch (wait.w) {
      case "find": {
        const to = motion.findChar(b, b.sel.head, c, wait.forward, wait.till);
        if (to !== undefined) this.take(b, [b.sel.head, to]);
        break;
      }
      case "replace":
        if (b.len === 0) break;
        this.edit(b);
        ops.replace(b, c);
        break;
      case "surroundAdd":
        if (b.len === 0) break;
        this.edit(b);
        ops.surroundAdd(b, c);
        break;
      case "surroundDelete":
        this.edit(b);
        if (!ops.surroundDelete(b, c)) {
          b.dropEmptyCheckpoint();
          return { a: "notice", text: `no ${c} around the selection` };
        }
        break;
      case "object": {
        if (c === "f") return { a: "syntax", op: wait.around ? "aroundFunction" : "inFunction" };
        const found = object(b, c, wait.around);
        if (!found) return { a: "notice", text: `no ${c} object here` };
        this.take(b, found);
        break;
      }
      case "surroundReplace":
        if (wait.from === undefined) {
          this.awaiting = { w: "surroundReplace", from: c };
          break;
        }
        this.edit(b);
        if (!ops.surroundReplace(b, wait.from, c)) {
          b.dropEmptyCheckpoint();
          return { a: "notice", text: `no ${wait.from} around the selection` };
        }
        break;
    }
    return NONE;
  }

  /** Takes the syntax tree's answer to a `syntax` action: chars `from..=to`, if any. */
  syntaxFound(b: Buffer, op: SyntaxOp, range: [number, number] | undefined): Action {
    if (!range) {
      if (op === "grow") return { a: "notice", text: "no syntax node around the selection" };
      if (op === "shrink") return NONE;
      return { a: "notice", text: "no f object here" };
    }
    if (op === "grow") {
      this.grown.push(b.sel);
      b.sel = new Sel(range[0], range[1]);
    } else if (op === "shrink") b.sel = new Sel(range[0], range[1]);
    else this.take(b, range);
    return NONE;
  }

  /** Searches for `needle` from the cursor, and keeps it for `n` and `N`. */
  searchFor(b: Buffer, needle: string) {
    this.search = needle;
    return this.nextMatch(b, true);
  }

  private nextMatch(b: Buffer, forward: boolean): Action {
    const needle = this.search;
    if (needle === undefined) return NONE;
    const at = forward ? b.sel.to() : b.sel.from();
    const found = motion.search(b, at, needle, forward);
    if (!found) return { a: "notice", text: `no match for ${needle}` };
    this.take(b, [found[0], found[1] - 1]);
    return NONE;
  }

  /** Puts `text` in: at the cursor while inserting, else after the selection. */
  putText(b: Buffer, text: string) {
    if (this.mode === "insert") {
      const at = b.sel.head;
      b.insert(at, text);
      b.sel = Sel.point(at + text.length);
    } else this.pasteText(b, text);
  }

  pasteText(b: Buffer, text: string) {
    this.edit(b);
    ops.paste(b, { text, lines: text.endsWith("\n") }, true);
    b.dropEmptyCheckpoint();
  }

  enterInsert(b: Buffer, at: number) {
    b.checkpoint();
    b.sel = Sel.point(at);
    this.startInsert(b);
  }

  private startInsert(b: Buffer) {
    this.insertedAt = b.version;
    this.mode = "insert";
  }

  /** Opens a line below or above the selection's, with its indent, to type into. */
  private openLine(b: Buffer, below: boolean) {
    b.checkpoint();
    const line = below ? b.lineOf(b.sel.to()) : b.lineOf(b.sel.from());
    const ind = b.indent(line);
    let at: number;
    if (below) {
      const end = b.lineEnd(line);
      b.insert(end, `\n${ind}`);
      at = end + 1 + ind.length;
    } else {
      const start = b.lineStart(line);
      b.insert(start, `${ind}\n`);
      at = start + ind.length;
    }
    b.sel = Sel.point(at);
    this.startInsert(b);
  }

  /** Leaves insert mode: back onto the last char typed, as vim does. */
  leaveInsert(b: Buffer) {
    renumber(b);
    const at = b.sel.head;
    b.dropEmptyCheckpoint();
    this.mode = "normal";
    const typed = b.version !== this.insertedAt;
    const back = typed && at > b.lineStart(b.lineOf(at));
    b.sel = Sel.point(back ? at - 1 : at);
    b.clamp();
  }

  private insertKey(b: Buffer, key: Key) {
    const at = b.sel.head;
    const k = key.key;
    if (k === "Escape") return this.leaveInsert(b);
    if (k === "w" && key.ctrl) {
      const from = motion.wordBackFrom(b, at);
      b.remove(from, at);
      b.sel = Sel.point(from);
    } else if (k.length === 1 && !key.ctrl) {
      b.insert(at, k);
      b.sel = Sel.point(at + 1);
    } else if (k === "Tab" && key.shift) {
      this.markdown(b, mdSmart);
    } else if (k === "Tab") {
      const tab = motion.indentUnit(b);
      b.insert(at, tab);
      b.sel = Sel.point(at + tab.length);
    } else if (k === "Enter") {
      if (!this.markdown(b, mdEnter)) {
        // Not more indent than the cursor is into the line.
        const into = at - b.lineStart(b.lineOf(at));
        const ind = b.indent(b.lineOf(at)).slice(0, into);
        b.insert(at, `\n${ind}`);
        b.sel = Sel.point(at + 1 + ind.length);
      }
    } else if (k === "Backspace" && at > 0) {
      b.remove(at - 1, at);
      b.sel = Sel.point(at - 1);
    } else if (k === "Delete" && at < b.len) b.remove(at, at + 1);
    else if (k === "ArrowLeft") b.sel = Sel.point(Math.max(at - 1, 0));
    else if (k === "ArrowRight") b.sel = Sel.point(Math.min(at + 1, b.len));
    else if (k === "Home") b.sel = Sel.point(b.lineStart(b.lineOf(at)));
    else if (k === "End") b.sel = Sel.point(b.lineEnd(b.lineOf(at)));
    else if (k === "ArrowUp" || k === "ArrowDown") {
      const [to, column] = motion.vertical(b, at, k === "ArrowUp" ? -1 : 1, b.column);
      b.column = column;
      b.sel = Sel.point(to);
      return;
    }
    b.column = undefined;
  }
}

/** Makes a markdown edit's changes and puts the cursor where it says. */
function apply(b: Buffer, edit: MdEdit) {
  for (const c of [...edit.changes].reverse()) {
    b.remove(c.from, c.to);
    b.insert(c.from, c.insert);
  }
  b.sel = Sel.point(Math.min(edit.cursor, b.len));
}

/** Fixes the numbers of the ordered list at the cursor, in a markdown file. */
function renumber(b: Buffer) {
  if (!isMarkdown(b.name)) return;
  const edit = mdRenumber(b.text, b.sel.head);
  if (!edit) return;
  const sel = b.sel;
  apply(b, edit);
  b.sel = sel;
  b.clamp();
}
