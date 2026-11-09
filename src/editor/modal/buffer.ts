/** Text, its selection, and its undo history, for the modal keys. Mirrors
 * `crates/lgtm/src/edit/buffer.rs`; positions count UTF-16 units, as CodeMirror's do. */

/** A selection: the chars from `anchor` to `head`, both included. `head` is the cursor. In
 * insert mode `head` is the gap typing goes into, before that char. */
export class Sel {
  constructor(
    readonly anchor: number,
    readonly head: number,
  ) {}

  static point(at: number) {
    return new Sel(at, at);
  }

  from() {
    return Math.min(this.anchor, this.head);
  }

  to() {
    return Math.max(this.anchor, this.head);
  }

  equals(o: Sel) {
    return o.anchor === this.anchor && o.head === this.head;
  }
}

interface State {
  text: string;
  sel: Sel;
}

export class Buffer {
  sel = Sel.point(0);
  /** Bumps on every change to the text, for what's derived from it. */
  version = 0;
  /** The column `j` and `k` keep to across shorter lines. */
  column: number | undefined;
  private undoStack: State[] = [];
  private redoStack: State[] = [];
  private starts: number[] | undefined;

  constructor(
    private _text: string,
    /** Its path, for what depends on the language. */
    readonly name: string,
  ) {}

  get text() {
    return this._text;
  }

  /** Takes text changed outside the keys, keeping the history. */
  setText(text: string) {
    if (text === this._text) return;
    this._text = text;
    this.starts = undefined;
    this.version++;
    this.clamp();
  }

  /** Marks where an undo step starts: undo comes back to the text and selection now. */
  checkpoint() {
    this.undoStack.push({ text: this._text, sel: this.sel });
    this.redoStack = [];
  }

  /** Drops the last checkpoint when nothing changed after it. */
  dropEmptyCheckpoint() {
    if (this.undoStack.at(-1)?.text === this._text) this.undoStack.pop();
  }

  undo() {
    return this.step(true);
  }

  redo() {
    return this.step(false);
  }

  private step(back: boolean) {
    const [from, to] = back ? [this.undoStack, this.redoStack] : [this.redoStack, this.undoStack];
    const state = from.pop();
    if (!state) return false;
    to.push({ text: this._text, sel: this.sel });
    this.replaceAll(state.text);
    this.sel = state.sel;
    this.clamp();
    return true;
  }

  private replaceAll(text: string) {
    this._text = text;
    this.starts = undefined;
    this.version++;
  }

  /** Inserts `s` at `at`. */
  insert(at: number, s: string) {
    this.replaceAll(this._text.slice(0, at) + s + this._text.slice(at));
  }

  /** Removes `from..to`. */
  remove(from: number, to: number) {
    if (from < to) this.replaceAll(this._text.slice(0, from) + this._text.slice(to));
  }

  get len() {
    return this._text.length;
  }

  char(i: number): string | undefined {
    return i >= 0 && i < this.len ? this._text[i] : undefined;
  }

  private lineStarts() {
    if (!this.starts) {
      const starts = [0];
      for (let i = 0; i < this._text.length; i++) if (this._text[i] === "\n") starts.push(i + 1);
      this.starts = starts;
    }
    return this.starts;
  }

  /** How many lines the text has, counting the empty one after a last newline. */
  get lineCount() {
    return this.lineStarts().length;
  }

  lineOf(i: number) {
    const starts = this.lineStarts();
    const at = Math.min(i, this.len);
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= at) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  }

  lineStart(line: number) {
    const starts = this.lineStarts();
    return line < starts.length ? starts[line] : this.len;
  }

  /** Where `line` ends: its newline, or the text's end. */
  lineEnd(line: number) {
    const next = this.lineStart(Math.min(line + 1, this.lineCount));
    return line + 1 < this.lineCount ? next - 1 : this.len;
  }

  /** The last line that has text or follows a newline the text doesn't end on. A text
   * ending in a newline has no line after it. */
  lastLine() {
    const lines = this.lineCount;
    return lines > 1 && this.lineStart(lines - 1) === this.len ? lines - 2 : lines - 1;
  }

  /** The last char a normal-mode cursor can be on. */
  lastChar() {
    return Math.max(this.len - 1, 0);
  }

  /** The selection kept inside the text. */
  clamp() {
    const max = this.lastChar();
    this.sel = new Sel(Math.min(this.sel.anchor, max), Math.min(this.sel.head, max));
  }

  slice(from: number, to: number) {
    return this._text.slice(Math.min(from, this.len), Math.min(to, this.len));
  }

  /** `line`'s leading spaces and tabs. */
  indent(line: number) {
    const text = this._text.slice(this.lineStart(line), this.lineEnd(line));
    return text.match(/^[ \t]*/)?.[0] ?? "";
  }
}
