import { useMemo, useState } from "react";
import type { CheckState } from "../bindings";
import { type FlatFile, hunkKey } from "./model";
import { toggled } from "./tree";

/** Which files and hunks are collapsed, and which gap bars were clicked open. */
export function useCollapse(files: FlatFile[], checks: CheckState[]) {
  const [collapsedHunks, setCollapsedHunks] = useState<ReadonlySet<string>>(new Set());
  /** Files opened or closed by hand, by path. Others follow their check and skip status. */
  const [fileOpen, setFileOpen] = useState<ReadonlyMap<string, boolean>>(new Map());
  const [revealed, setRevealed] = useState<ReadonlySet<string>>(new Set());
  const collapsedFiles = useMemo(() => {
    const out = new Set<string>();
    files.forEach((f, i) => {
      const byDefault = checks[i] === "checked" || f.dto.skipped;
      if (!(fileOpen.get(f.dto.path) ?? !byDefault)) out.add(f.dto.path);
    });
    return out;
  }, [files, checks, fileOpen]);

  function toggleHunk(file: number, hunk: number) {
    setCollapsedHunks(toggled(collapsedHunks, hunkKey(files[file].dto.path, hunk)));
  }

  function toggleFile(file: number) {
    const path = files[file].dto.path;
    setFileOpen(new Map(fileOpen).set(path, collapsedFiles.has(path)));
  }

  return {
    collapsedFiles,
    collapsedHunks,
    revealed,
    toggleHunk,
    toggleFile,
    reveal: (gap: string) => setRevealed(new Set(revealed).add(gap)),
    clearRevealed: () => setRevealed(new Set()),
    reset() {
      setCollapsedHunks(new Set());
      setFileOpen(new Map());
      setRevealed(new Set());
    },
    /** Checking collapses a file and unchecking expands it, whatever was done by hand. */
    followChecks(fileIdxs: number[]) {
      if (!fileIdxs.length) return;
      const next = new Map(fileOpen);
      for (const f of fileIdxs) next.delete(files[f]?.dto.path ?? "");
      setFileOpen(next);
    },
  };
}
