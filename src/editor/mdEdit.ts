/** Markdown editing: lists and quotes that go on at Enter, ordered lists that keep their
 * numbers, checkboxes, and the smart key for tables and footnotes. The TUI's editor does the
 * same in `crates/lgtm/src/edit/markdown.rs`. */

/** Text that replaces `from..to`. */
export interface Change {
  from: number;
  to: number;
  insert: string;
}

/** Changes that don't overlap, in order, and where the cursor goes once they're made. */
export interface MdEdit {
  changes: Change[];
  cursor: number;
}

export const isMarkdown = (path: string) => /\.(md|markdown|mdx)$/i.test(path);

/** `text` with the edit's changes made. */
export function applyEdit(text: string, edit: MdEdit): string {
  let out = "";
  let at = 0;
  for (const c of edit.changes) {
    out += text.slice(at, c.from) + c.insert;
    at = c.to;
  }
  return out + text.slice(at);
}

/** The text's lines, without their newlines, and where each starts. */
class Lines {
  text: string[];
  starts: number[] = [];
  len: number;
  constructor(src: string) {
    this.text = src.split("\n");
    this.len = src.length;
    let at = 0;
    for (const l of this.text) {
      this.starts.push(at);
      at += l.length + 1;
    }
  }
  of(at: number): number {
    let n = 0;
    while (n + 1 < this.starts.length && this.starts[n + 1] <= at) n++;
    return n;
  }
  end(n: number): number {
    return this.starts[n] + this.text[n].length;
  }
}

type Marker =
  | { kind: "bullet"; c: string }
  | { kind: "number"; n: number; d: string }
  | { kind: "letter"; c: string; d: string };

function nextMarker(m: Marker): Marker {
  if (m.kind === "number") return { ...m, n: m.n + 1 };
  if (m.kind === "letter" && m.c !== "z" && m.c !== "Z") {
    return { ...m, c: String.fromCharCode(m.c.charCodeAt(0) + 1) };
  }
  return m;
}

/** Whether `b` is in the same ordered list as `a`. */
function sameKind(a: Marker, b: Marker): boolean {
  if (a.kind === "number" && b.kind === "number") return a.d === b.d;
  if (a.kind === "letter" && b.kind === "letter") {
    return a.d === b.d && (a.c === a.c.toLowerCase()) === (b.c === b.c.toLowerCase());
  }
  return false;
}

function markerText(m: Marker): string {
  if (m.kind === "bullet") return m.c;
  return m.kind === "number" ? `${m.n}${m.d}` : `${m.c}${m.d}`;
}

const sameMarker = (a: Marker, b: Marker) => markerText(a) === markerText(b);

/** A list item's line, split up. */
interface Item {
  indent: string;
  marker: Marker;
  /** Where the marker ends. */
  markerEnd: number;
  gap: string;
  /** The checkbox's mark, " " or "x". */
  check?: string;
  /** Where the item's own text starts. */
  body: number;
}

const ITEM = /^([ \t]*)([-*+]|\d{1,9}[.)]|[A-Za-z][.)])( *)/;

function item(line: string): Item | undefined {
  const m = ITEM.exec(line);
  if (!m) return;
  const [all, indent, mark, spaces] = m;
  if (!spaces && all.length < line.length) return;
  const marker: Marker = /^[-*+]$/.test(mark)
    ? { kind: "bullet", c: mark }
    : /^\d/.test(mark)
      ? { kind: "number", n: Number(mark.slice(0, -1)), d: mark.slice(-1) }
      : { kind: "letter", c: mark[0], d: mark[1] };
  const it: Item = {
    indent,
    marker,
    markerEnd: indent.length + mark.length,
    gap: spaces || " ",
    body: all.length,
  };
  const box = /^\[([ xX])\](?= |$) */.exec(line.slice(all.length));
  if (box) {
    it.check = box[1] === " " ? " " : "x";
    it.body += box[0].length;
  }
  return it;
}

/** The `>` marks starting a quote's line, with their spaces. */
function quotePrefix(line: string): string | undefined {
  return /^(?:[ \t]*> ?)+/.exec(line)?.[0];
}

/** Enter at `at`: a list item or quote goes on to a new line with the same marker, the next
 * number or letter, and an empty checkbox where it had one. Enter on an empty one ends the
 * list or quote. Enter at the end of a table row starts a new row, and of a table's header,
 * its delimiter row first. */
export function mdEnter(text: string, at: number): MdEdit | undefined {
  const lines = new Lines(text);
  const l = lines.of(at);
  const start = lines.starts[l];
  const col = at - start;
  const line = lines.text[l];
  const it = item(line);
  if (it && col >= it.body) {
    if (!line.slice(it.body).trim()) {
      return { changes: [{ from: start, to: lines.end(l), insert: "" }], cursor: start };
    }
    const next = nextMarker(it.marker);
    const prefix = `${it.indent}${markerText(next)}${it.gap}${it.check ? "[ ] " : ""}`;
    return {
      changes: [
        { from: at, to: at, insert: `\n${prefix}` },
        ...renumberAfter(lines, l, it.indent, nextMarker(next), next),
      ],
      cursor: at + 1 + prefix.length,
    };
  }
  const prefix = quotePrefix(line);
  if (prefix && col >= prefix.length) {
    if (!line.slice(prefix.length).trim()) {
      return { changes: [{ from: start, to: lines.end(l), insert: "" }], cursor: start };
    }
    return {
      changes: [{ from: at, to: at, insert: `\n${prefix}` }],
      cursor: at + 1 + prefix.length,
    };
  }
  if (isRow(line) && col === line.length) {
    const indent = /^ */.exec(line)?.[0] ?? "";
    const headed = l > 0 && isRow(lines.text[l - 1]);
    const delimited = l + 1 < lines.text.length && isDelimiter(lines.text[l + 1]);
    let insert = "\n";
    if (!headed && !delimited) {
      const row = cells(line).map((c) => "-".repeat(Math.max(width(c, true), 3)));
      insert += `${indent}| ${row.join(" | ")} |\n`;
    }
    insert += `${indent}| `;
    return { changes: [{ from: at, to: at, insert }], cursor: at + insert.length };
  }
}

const leading = (s: string) => /^[ \t]*/.exec(s)?.[0] ?? "";
const deeperThan = (s: string, indent: string) => {
  const ws = leading(s);
  return ws.length > indent.length && ws.startsWith(indent);
};

/** Fixes the numbers of the items after line `l` in its ordered list, which has `indent`;
 * the first should be `expect`, and `first` is the marker before them. */
function renumberAfter(
  lines: Lines,
  l: number,
  indent: string,
  expect: Marker,
  first: Marker,
): Change[] {
  const changes: Change[] = [];
  if (first.kind === "bullet") return changes;
  for (let n = l + 1; n < lines.text.length; n++) {
    const line = lines.text[n];
    if (!line.trim() || deeperThan(line, indent)) continue;
    const it = item(line);
    if (!it || it.indent !== indent || !sameKind(first, it.marker)) break;
    if (!sameMarker(it.marker, expect)) {
      const from = lines.starts[n] + it.indent.length;
      changes.push({ from, to: lines.starts[n] + it.markerEnd, insert: markerText(expect) });
    }
    expect = nextMarker(expect);
  }
  return changes;
}

/** Fixes the numbers of the ordered list the line at `at` is in, from its first item. */
export function mdRenumber(text: string, at: number): MdEdit | undefined {
  const lines = new Lines(text);
  let l = lines.of(at);
  let it = item(lines.text[l]);
  // Up to the item the line belongs to.
  while (!it) {
    const line = lines.text[l];
    if (l === 0 || !(!line.trim() || /^[ \t]/.test(line))) return;
    l--;
    it = item(lines.text[l]);
  }
  // Up to the list's first item.
  for (let n = l - 1; n >= 0; n--) {
    const line = lines.text[n];
    if (!line.trim() || deeperThan(line, it.indent)) continue;
    const up = item(line);
    if (!up || up.indent !== it.indent || !sameKind(up.marker, it.marker)) break;
    l = n;
    it = up;
  }
  const changes = renumberAfter(lines, l, it.indent, nextMarker(it.marker), it.marker);
  return changes.length ? { changes, cursor: at } : undefined;
}

/** Checks or unchecks the checkbox on `at`'s line, or gives a list item without one an
 * empty one. */
export function mdToggleCheck(text: string, at: number): MdEdit | undefined {
  const lines = new Lines(text);
  const l = lines.of(at);
  const it = item(lines.text[l]);
  if (!it) return;
  const box = lines.starts[l] + it.markerEnd + it.gap.length;
  if (it.check) {
    const insert = it.check === " " ? "x" : " ";
    return { changes: [{ from: box + 1, to: box + 2, insert }], cursor: at };
  }
  return {
    changes: [{ from: box, to: box, insert: "[ ] " }],
    cursor: at >= box ? at + 4 : at,
  };
}

/** Checks or unchecks the checkbox on line `line`, from 1, for the rendered view. */
export function toggleLine(text: string, line: number): string | undefined {
  const lines = new Lines(text);
  const at = lines.starts[line - 1];
  if (at === undefined || !item(lines.text[line - 1])?.check) return;
  const edit = mdToggleCheck(text, at);
  return edit && applyEdit(text, edit);
}

/** The smart key: on a footnote reference, goes to its note, writing one at the bottom when
 * there's none; on a note, goes back to its reference; after a bare `[^`, numbers a new
 * footnote. In a table, lines up the columns and moves to the next cell, adding it. */
export function mdSmart(text: string, at: number): MdEdit | undefined {
  return footnote(text, at) ?? table(text, at);
}

/** A footnote note's label, when `line` starts one. */
const noteLabel = (line: string) => /^\[\^([^\]\s]+)\]:/.exec(line)?.[1];

/** The footnote references in `line`: their labels and spans. */
function refs(line: string): { label: string; from: number; to: number }[] {
  const out = [];
  for (const m of line.matchAll(/\[\^([^\]\s]+)\](?!:)/g)) {
    out.push({ label: m[1], from: m.index, to: m.index + m[0].length });
  }
  return out;
}

function footnote(text: string, at: number): MdEdit | undefined {
  const lines = new Lines(text);
  const l = lines.of(at);
  const line = lines.text[l];
  const col = at - lines.starts[l];
  const own = noteLabel(line);
  if (own) {
    for (let n = 0; n < lines.text.length; n++) {
      const r = refs(lines.text[n]).find((r) => r.label === own);
      if (r) return { changes: [], cursor: lines.starts[n] + r.to };
    }
    return;
  }
  let label = refs(line).find((r) => r.from <= col && col <= r.to)?.label;
  let fill: string | undefined;
  if (!label) {
    if (col < 2 || line.slice(col - 2, col) !== "[^" || line[col] === "]") return;
    const nums = lines.text.flatMap((t) => [
      ...refs(t).map((r) => Number(r.label)),
      Number(noteLabel(t)),
    ]);
    label = String(Math.max(0, ...nums.filter((n) => Number.isInteger(n))) + 1);
    fill = `${label}]`;
  }
  for (let n = 0; n < lines.text.length; n++) {
    const note = lines.text[n];
    if (noteLabel(note) === label) {
      const head = label.length + 4;
      const skip = /^ */.exec(note.slice(head))?.[0].length ?? 0;
      return { changes: [], cursor: lines.starts[n] + head + skip };
    }
  }
  // A new note at the bottom, after a blank line unless it follows other notes.
  const changes: Change[] = [];
  if (fill) changes.push({ from: at, to: at, insert: fill });
  const endsNewline = text.endsWith("\n");
  let last = lines.text.length - 1;
  if (endsNewline && last > 0) last--;
  let note = "";
  if (!endsNewline && text) note += "\n";
  if (noteLabel(lines.text[last]) === undefined && text && lines.text[last].trim()) note += "\n";
  note += `[^${label}]: `;
  const cursor = text.length + (fill?.length ?? 0) + note.length;
  if (endsNewline) note += "\n";
  changes.push({ from: text.length, to: text.length, insert: note });
  return { changes, cursor };
}

const isRow = (line: string) => line.trimStart().startsWith("|");
const isDelimiter = (line: string) =>
  isRow(line) && line.includes("-") && /^[|\-: ]*$/.test(line.trim());

/** A row's cells, as written between its pipes. Escaped pipes stay in their cell. */
function cells(line: string): string[] {
  let t = line.trimStart();
  if (t.startsWith("|")) t = t.slice(1);
  const out = [""];
  let escaped = false;
  for (const c of t) {
    if (c === "|" && !escaped) out.push("");
    else out[out.length - 1] += c;
    escaped = c === "\\" && !escaped;
  }
  // A closing pipe leaves an empty cell after it.
  if (out.length > 1 && !out[out.length - 1].trim() && t.trimEnd().endsWith("|")) out.pop();
  return out;
}

/** How wide a cell's text is. A header cell's spaces count, since they set its column's
 * width; one space each side is the pipe's padding. */
function width(cell: string, header: boolean): number {
  const text = cell.trim().length;
  if (!header) return text;
  let w = cell.length;
  if (cell.startsWith(" ")) w--;
  if (cell.endsWith(" ") && w > 0) w--;
  return Math.max(w, text);
}

function table(text: string, at: number): MdEdit | undefined {
  const lines = new Lines(text);
  const l = lines.of(at);
  const line = lines.text[l];
  if (!isRow(line)) return;
  const col = at - lines.starts[l];
  let first = l;
  while (first > 0 && isRow(lines.text[first - 1])) first--;
  let last = l;
  while (last + 1 < lines.text.length && isRow(lines.text[last + 1])) last++;
  // The cell the cursor is in.
  const before = line.slice(0, col);
  const cell = cells(before).length - 1;
  if (!(first < last && isDelimiter(lines.text[first + 1]))) {
    const pad = before.endsWith(" ") ? "| " : " | ";
    return { changes: [{ from: at, to: at, insert: pad }], cursor: at + pad.length };
  }
  const indent = /^ */.exec(lines.text[first])?.[0] ?? "";
  const rows = lines.text.slice(first, last + 1).map(cells);
  const row = l - first;
  if (row === 1) return;
  const atEnd = cell + 1 >= Math.max(rows[0].length, rows[row].length);
  if (!atEnd) while (rows[row].length < cell + 2) rows[row].push("");
  const count = Math.max(...rows.map((r) => r.length));
  const widths = Array.from({ length: count }, (_, c) =>
    Math.max(
      3,
      ...rows.flatMap((cs, r) => (r === 1 || cs[c] === undefined ? [] : [width(cs[c], r === 0)])),
    ),
  );
  const out: string[] = [];
  let cursor: number | undefined;
  rows.forEach((cs, r) => {
    let s = `${indent}|`;
    if (r === 1) {
      for (let c = 0; c < count; c++) {
        const d = cs[c]?.trim() ?? "";
        const left = d.startsWith(":");
        const right = d.length > 1 && d.endsWith(":");
        const dashes = "-".repeat(widths[c] - Number(left) - Number(right));
        s += ` ${left ? ":" : ""}${dashes}${right ? ":" : ""} |`;
      }
      out.push(s);
      return;
    }
    const offset = () => out.reduce((n, o) => n + o.length + 1, 0) + s.length;
    for (let c = 0; c < cs.length; c++) {
      const t = cs[c].trim();
      s += " ";
      if (r === row && c === cell + 1) {
        cursor = offset();
        // The cell just started stays open to type into.
        if (!t && c + 1 === cs.length) break;
      }
      s += `${t.padEnd(widths[c])} |`;
    }
    if (r === row && cursor === undefined) {
      // The last cell: a new row to type into.
      s += `\n${indent}| `;
      cursor = offset();
    }
    out.push(s);
  });
  const from = lines.starts[first];
  return {
    changes: [{ from, to: lines.end(last), insert: out.join("\n") }],
    cursor: from + (cursor ?? 0),
  };
}
