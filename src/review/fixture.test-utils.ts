import type { CodeSymbol, FileDto, ReviewDto, RowDto } from "../bindings";
import type { Layout } from "./model";

/** Rows from a compact spec: " ctx", "-removed", "+added". Line numbers are assigned, and
 * removed rows are replaced when their change also adds rows. */
export function rows(spec: string[], oldStart = 1, newStart = 1): RowDto[] {
  let o = oldStart;
  let n = newStart;
  return spec.map((s, i) => {
    const text = s.slice(1);
    const row = { text, hl: null, replaced: false };
    if (s[0] === "-") {
      let j = i;
      while (spec[j]?.[0] === "-") j++;
      return {
        ...row,
        kind: "removed",
        oldLine: o++,
        newLine: null,
        replaced: spec[j]?.[0] === "+",
      };
    }
    if (s[0] === "+") return { ...row, kind: "added", oldLine: null, newLine: n++ };
    return { ...row, kind: "context", oldLine: o++, newLine: n++ };
  });
}

export function sym(
  name: string,
  startLine: number,
  endLine: number,
  hasError = false,
): CodeSymbol {
  return { name, kind: "function", startLine, endLine, hasError };
}

export function file(path: string, hunks: RowDto[][], newSymbols: CodeSymbol[] = []): FileDto {
  return {
    path,
    oldPath: null,
    status: "modified",
    binary: false,
    conflict: false,
    oldExec: false,
    newExec: false,
    skipped: false,
    hunks: hunks.map((r) => ({
      oldStart: 1,
      oldLines: 0,
      newStart: 1,
      newLines: 0,
      section: "",
      rows: r,
    })),
    lineCount: 0,
    oldSymbols: [],
    newSymbols,
  };
}

export function review(files: FileDto[]): ReviewDto {
  return {
    id: 1,
    revset: "@",
    view: { kind: "combined" },
    from: null,
    to: "x",
    head: "x",
    interdiff: false,
    files,
    state: {
      marks: [],
      checks: files.map(() => "unchecked"),
      amber: [],
      comments: [],
      threads: [],
    },
    changes: [],
    removedChanges: [],
    previousSession: null,
    stale: null,
  };
}

export function layout(over: Partial<Layout> = {}): Layout {
  return {
    mode: "unified",
    collapsedFiles: new Set(),
    collapsedHunks: new Set(),
    filter: "all",
    rowState: () => "unmarked",
    done: (c) => c !== "red",
    revealed: new Set(),
    ...over,
  };
}
