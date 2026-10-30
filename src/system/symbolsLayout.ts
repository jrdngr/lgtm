import type { SymbolsDto, SymNodeDto } from "../bindings";
import type { Box } from "./layout";

/** A symbol: one of the component's, or one outside it that calls in or is called. */
export interface SymBox extends Box {
  key: string;
  sym: SymNodeDto;
  side: "in" | "caller" | "call";
}

/** The outside symbols of one component on one side, as one chip. */
export interface ChipBox extends Box {
  key: string;
  component: string;
  side: "caller" | "call";
  ids: number[];
}

export interface FileBox extends Box {
  path: string;
}

export interface SymLayout {
  /** The component's symbols, then the outside ones, which sit on their chip. */
  syms: SymBox[];
  chips: ChipBox[];
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
/** Between the column titles and the first chip. */
const TITLE = 36;
const SYM_H = 32;
const SYM_STEP = 40;
const CHIP_H = 34;
const CHIP_STEP = 48;
const FILE_W = 240;
const FILE_HEAD = 30;
const FILE_PAD = 14;
const FILE_GAP = 24;
const INSET = 22;

export const symKey = (id: number) => `s${id}`;
const chipKey = (side: ChipBox["side"], component: string) => `${side}:${component}`;

/** How many columns of files the frame holds. */
const fileColumns = (n: number) => (n <= 2 ? Math.max(1, n) : n <= 6 ? 2 : 3);

const mean = (ns: number[]) => (ns.length ? ns.reduce((a, b) => a + b, 0) / ns.length : 0);

/** Callers left as one chip per component, the component's files with their symbols in its
 * frame, calls right. Each chip column is ordered by where its links land, so the faint lines
 * between them cross less. */
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
        key: symKey(sym.id),
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
  const column = (list: SymNodeDto[], side: ChipBox["side"], x: number) => {
    const groups = new Map<string, SymNodeDto[]>();
    for (const s of list) groups.set(s.component, [...(groups.get(s.component) ?? []), s]);
    const ends = (ids: Set<number>) =>
      dto.edges.flatMap((e) =>
        side === "caller" ? (ids.has(e.from) ? [e.to] : []) : ids.has(e.to) ? [e.from] : [],
      );
    const at = (g: SymNodeDto[]) =>
      mean(ends(new Set(g.map((s) => s.id))).map((id) => yOf.get(id) ?? 0));
    const chips = [...groups]
      .sort((a, b) => at(a[1]) - at(b[1]))
      .map(([component, g], i) => ({
        key: chipKey(side, component),
        component,
        side,
        ids: g.map((s) => s.id),
        x,
        y: PAD + TITLE + i * CHIP_STEP,
        w: COLUMN_W,
        h: CHIP_H,
      }));
    for (const c of chips) {
      for (const sym of groups.get(c.component) ?? []) {
        syms.push({ key: symKey(sym.id), sym, side, x: c.x, y: c.y, w: c.w, h: SYM_H });
      }
    }
    return chips;
  };
  const right = frame.x + frame.w + COLUMN_GAP;
  const chips = [...column(dto.callers, "caller", PAD), ...column(dto.calls, "call", right)];
  const bottom = Math.max(frame.y + frame.h, ...chips.map((c) => c.y + c.h));
  return {
    syms,
    chips,
    files,
    frame,
    callers: dto.callers.length ? { x: PAD, y: PAD } : undefined,
    calls: dto.calls.length ? { x: right, y: PAD } : undefined,
    width: (dto.calls.length ? right + COLUMN_W : frame.x + frame.w) + PAD,
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
