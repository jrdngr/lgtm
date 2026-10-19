import type { SymbolsDto, SymNodeDto } from "../bindings";
import type { Box } from "./layout";

export interface SymBox extends Box {
  sym: SymNodeDto;
  side: "in" | "caller" | "call";
}

export interface FileBox extends Box {
  path: string;
}

export interface SymLayout {
  syms: SymBox[];
  files: FileBox[];
  /** The component's frame around its files. */
  frame: Box;
  /** Where the Callers and Calls column titles go, when there are any. */
  callers?: { x: number; y: number };
  calls?: { x: number; y: number };
  width: number;
  height: number;
}

const PAD = 40;
const COLUMN_W = 180;
const COLUMN_GAP = 60;
/** Between the column titles and the first symbol. */
const TITLE = 36;
const SYM_H = 32;
const SYM_STEP = 40;
const FILE_W = 240;
const FILE_HEAD = 30;
const FILE_PAD = 14;
const FILE_GAP = 24;
const INSET = 22;

/** How many columns of files the frame holds. */
const fileColumns = (n: number) => (n <= 2 ? Math.max(1, n) : n <= 6 ? 2 : 3);

const mean = (ns: number[]) => (ns.length ? ns.reduce((a, b) => a + b, 0) / ns.length : 0);

/** Callers left, the component's files with their symbols in its frame, calls right. Each
 * side column is ordered by where its links land, so lines cross less. */
export function symbolsLayout(dto: SymbolsDto): SymLayout {
  const byFile = new Map<string, SymNodeDto[]>();
  for (const s of dto.symbols) {
    const list = byFile.get(s.path);
    if (list) list.push(s);
    else byFile.set(s.path, [s]);
  }
  const top = PAD + TITLE - 20;
  const left = dto.callers.length ? PAD + COLUMN_W + COLUMN_GAP : PAD;
  const cols = fileColumns(byFile.size);
  const heights = Array(cols).fill(0) as number[];
  const syms: SymBox[] = [];
  const files: FileBox[] = [];
  for (const [path, list] of byFile) {
    const col = heights.indexOf(Math.min(...heights));
    const x = left + INSET + col * (FILE_W + FILE_GAP);
    const y = top + INSET + heights[col];
    const h = FILE_HEAD + list.length * SYM_STEP + FILE_PAD - (SYM_STEP - SYM_H);
    files.push({ path, x, y, w: FILE_W, h });
    list.forEach((sym, i) => {
      syms.push({
        sym,
        side: "in",
        x: x + FILE_PAD,
        y: y + FILE_HEAD + i * SYM_STEP,
        w: FILE_W - FILE_PAD * 2,
        h: SYM_H,
      });
    });
    heights[col] += h + FILE_GAP;
  }
  const frame = {
    x: left,
    y: top,
    w: INSET * 2 + cols * FILE_W + (cols - 1) * FILE_GAP,
    h: INSET * 2 + Math.max(0, Math.max(...heights) - FILE_GAP),
  };

  const yOf = new Map(syms.map((b) => [b.sym.id, b.y]));
  const order = (list: SymNodeDto[], ends: (id: number) => number[]) =>
    [...list].sort(
      (a, b) =>
        mean(ends(a.id).map((e) => yOf.get(e) ?? 0)) - mean(ends(b.id).map((e) => yOf.get(e) ?? 0)),
    );
  const column = (list: SymNodeDto[], x: number, side: SymBox["side"]) =>
    list.forEach((sym, i) => {
      syms.push({ sym, side, x, y: PAD + TITLE + i * (SYM_H + 28), w: COLUMN_W, h: SYM_H });
    });
  const callers = order(dto.callers, (id) =>
    dto.edges.filter((e) => e.from === id).map((e) => e.to),
  );
  const calls = order(dto.calls, (id) => dto.edges.filter((e) => e.to === id).map((e) => e.from));
  column(callers, PAD, "caller");
  const right = frame.x + frame.w + COLUMN_GAP;
  column(calls, right, "call");
  const bottom = Math.max(frame.y + frame.h, ...syms.map((s) => s.y + s.h));
  return {
    syms,
    files,
    frame,
    callers: callers.length ? { x: PAD, y: PAD } : undefined,
    calls: calls.length ? { x: right, y: PAD } : undefined,
    width: (calls.length ? right + COLUMN_W : frame.x + frame.w) + PAD,
    height: bottom + PAD,
  };
}

/** `fn`, `type` and the like, for a symbol's kind badge. */
export const KIND_LABEL: Record<SymNodeDto["kind"], string> = {
  function: "fn",
  class: "type",
  module: "mod",
  section: "§",
};
