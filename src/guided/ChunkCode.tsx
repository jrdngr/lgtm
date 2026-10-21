import { useEffect, useState } from "react";
import type { GuidedRange } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";
import { SIGN } from "../model/sign";
import { Code } from "../review/Cell";
import { type FlatFile, markKey } from "../review/model";
import { inRanges, type Segment } from "./model";

/** Rows picked for a comment: changed rows of one file. */
export interface LineSel {
  file: number;
  rows: number[];
}

interface Props {
  files: FlatFile[];
  segments: Segment[];
  /** Rows PR threads are on, by `markKey`. */
  ringed?: ReadonlySet<string>;
  /** A step's lines, lit while it's the one shown. */
  lit?: GuidedRange[];
  sel?: LineSel;
  /** Clicked a changed row; `extend` with shift held. */
  onRow?(file: number, row: number, extend: boolean): void;
}

/** A chunk's diff, one block per run of rows, under its file's path. */
export function ChunkCode({ files, segments, ringed, lit, sel, onRow }: Props) {
  return (
    <>
      {segments.map((s) => {
        const f = files[s.file];
        const rows = s.rows.map((r) => f.rows[r]);
        const add = rows.filter((r) => r.kind === "added").length;
        const del = rows.filter((r) => r.kind === "removed").length;
        return (
          <div key={`${s.file}:${s.rows[0]}`} className="gd-code">
            <div className="gd-fh">
              <span className="mono">{s.path}</span>
              {add > 0 && <span className="gd-add">+{add}</span>}
              {del > 0 && <span className="gd-del">−{del}</span>}
            </div>
            {s.rows.map((row) => {
              const r = f.rows[row];
              const changed = r.kind !== "context";
              const cls = [
                "gd-ln",
                `kind-${r.kind}`,
                ringed?.has(markKey(s.file, row)) && "ring",
                lit && inRanges(lit, s.path, r.newLine) && "lit",
                sel?.file === s.file && sel.rows.includes(row) && "sel",
                changed && onRow && "pickable",
              ].filter(Boolean);
              return (
                // biome-ignore lint/a11y/useKeyWithClickEvents: rows pick with the mouse; keys act on the chunk
                <div
                  key={row}
                  className={cls.join(" ")}
                  onClick={changed && onRow ? (e) => onRow(s.file, row, e.shiftKey) : undefined}
                >
                  <span className="n">{r.newLine ?? r.oldLine ?? ""}</span>
                  <span className="s">{SIGN[r.kind]}</span>
                  <Code row={r} />
                </div>
              );
            })}
          </div>
        );
      })}
    </>
  );
}

interface Line {
  line: number;
  text: string;
  hl: [number | null, number][] | null;
}

/** Unchanged lines read alongside a chunk, from the workspace's copy of their file. */
export function RelatedCode({
  workspace,
  range,
  lit,
}: {
  workspace: WorkspaceName;
  range: GuidedRange;
  lit?: GuidedRange[];
}) {
  const [lines, setLines] = useState<Line[]>();
  const [error, setError] = useState<string>();
  useEffect(() => {
    let live = true;
    unwrap(commands.readFile(workspace, range.path))
      .then(async (text) => {
        const all = text.split("\n");
        const hl = await commands.highlightText(range.path, text).catch(() => []);
        const out: Line[] = [];
        for (let n = range.start; n <= Math.min(range.end, all.length); n++)
          out.push({ line: n, text: all[n - 1], hl: hl[n - 1] ?? null });
        if (live) setLines(out);
      })
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [workspace, range.path, range.start, range.end]);
  return (
    <div className="gd-code">
      <div className="gd-fh">
        <button
          type="button"
          className="gd-link mono"
          onClick={() => openFile(workspace, range.path, range.start)}
        >
          {range.path}
        </button>
        <span className="gd-tag">Related</span>
      </div>
      {error && <div className="gd-ln">{error}</div>}
      {lines?.map((l) => (
        <div
          key={l.line}
          className={`gd-ln kind-context${lit && inRanges(lit, range.path, l.line) ? " lit" : ""}`}
        >
          <span className="n">{l.line}</span>
          <span className="s" />
          <Code row={l} />
        </div>
      ))}
    </div>
  );
}
