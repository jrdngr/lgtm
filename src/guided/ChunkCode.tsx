import { Fragment, type ReactNode, useEffect, useState } from "react";
import type { GuidedRange, MarkColor } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";
import { SIGN } from "../model/sign";
import { Code } from "../review/Cell";
import { markVar } from "../review/marks";
import { type FlatFile, markKey } from "../review/model";
import { IconButton } from "../ui/icons";
import { inRanges, type Segment } from "./model";
import type { GuidedLines } from "./useGuidedLines";

interface Props {
  files: FlatFile[];
  segments: Segment[];
  /** A step's lines, lit while it's the one shown. */
  lit?: GuidedRange[];
  /** Picking and acting on lines; without it the code is read-only. */
  lines?: GuidedLines;
  /** A changed row's mark color, if it has one. */
  markOf?(file: number, row: number): MarkColor | undefined;
  /** What shows under a row, like the comments that end there. */
  after?(file: number, row: number): ReactNode;
  /** Files shown whole; with `onExpand`, each header can switch its file. */
  expanded?: ReadonlySet<number>;
  onExpand?(file: number): void;
}

/** A chunk's diff, one block per run of rows, under its file's path. */
export function ChunkCode({
  files,
  segments,
  lit,
  lines,
  markOf,
  after,
  expanded,
  onExpand,
}: Props) {
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
              {onExpand && (
                <IconButton
                  className="gd-expand"
                  icon={expanded?.has(s.file) ? "collapse" : "expand"}
                  label={expanded?.has(s.file) ? "Step lines" : "Whole file"}
                  onClick={() => onExpand(s.file)}
                />
              )}
            </div>
            {s.rows.map((row) => {
              const r = f.rows[row];
              const mark = r.kind === "context" ? undefined : markOf?.(s.file, row);
              const cls = [
                "gd-ln",
                `kind-${r.kind}`,
                mark && "marked",
                lit && inRanges(lit, s.path, r.newLine) && "lit",
                lines?.isPicked(s.file, row) && "sel",
                lines?.isHead(s.file, row) && "head",
                lines && "pickable",
              ].filter(Boolean);
              return (
                <Fragment key={row}>
                  <div
                    className={cls.join(" ")}
                    data-row={markKey(s.file, row)}
                    style={mark ? ({ "--m": markVar(mark) } as React.CSSProperties) : undefined}
                    onMouseDown={lines && ((e) => lines.onRowDown({ file: s.file, row }, e))}
                    onMouseEnter={lines && (() => lines.onRowEnter({ file: s.file, row }))}
                    onContextMenu={lines?.onRowMenu({ file: s.file, row })}
                  >
                    <span className="n">{r.newLine ?? r.oldLine ?? ""}</span>
                    <span className="s">{SIGN[r.kind]}</span>
                    <Code row={r} />
                  </div>
                  {after?.(s.file, row)}
                </Fragment>
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
