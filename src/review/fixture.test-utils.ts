import type { CodeSymbol, FileDto, ReviewDto, RowDto } from "../bindings";

/** Rows from a compact spec: " ctx", "-removed", "+added". Line numbers are assigned. */
export function rows(spec: string[], oldStart = 1, newStart = 1): RowDto[] {
  let o = oldStart;
  let n = newStart;
  return spec.map((s) => {
    const text = s.slice(1);
    if (s[0] === "-") return { kind: "removed", oldLine: o++, newLine: null, text, hl: null };
    if (s[0] === "+") return { kind: "added", oldLine: null, newLine: n++, text, hl: null };
    return { kind: "context", oldLine: o++, newLine: n++, text, hl: null };
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
    hunks: hunks.map((r) => ({
      oldStart: 1,
      oldLines: 0,
      newStart: 1,
      newLines: 0,
      section: "",
      rows: r,
    })),
    oldSymbols: [],
    newSymbols,
  };
}

export function review(files: FileDto[]): ReviewDto {
  return { id: 1, revset: "@", from: null, to: "x", files, marks: [] };
}
