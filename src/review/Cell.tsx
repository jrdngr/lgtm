import { memo, type ReactNode } from "react";
import { HIGHLIGHT_NAMES, type MarkColor, type MarkDto, type RowDto } from "../bindings";
import { SIGN } from "../model/sign";
import { type MarkSetup, markVar } from "./marks";
import { type FlatFile, markKey } from "./model";
import type { MarkIndex } from "./state";
import "./syntax.css";

const hlClass = HIGHLIGHT_NAMES.map((n) => `hl-${n.replace(".", "-")}`);

/** A line's code with its highlighting. Rows don't change while a review shows them, so it
 * renders again only for another row. */
export const Code = memo(function Code({ row }: { row: Pick<RowDto, "text" | "hl"> }) {
  if (!row.hl) return <span className="code">{row.text}</span>;
  let pos = 0;
  return (
    <span className="code">
      {row.hl.map(([cls, len]) => {
        const start = pos;
        pos += len;
        const piece = row.text.slice(start, pos);
        return cls === null ? (
          piece
        ) : (
          <span key={start} className={hlClass[cls]}>
            {piece}
          </span>
        );
      })}
      {row.text.slice(pos)}
    </span>
  );
});

function markTitle(m: MarkDto | undefined, setup: MarkSetup) {
  if (!m) return undefined;
  const when = m.createdAt === null ? "" : ` · ${new Date(m.createdAt).toLocaleString()}`;
  return `${setup.label(m.color)} in ${m.review}${when}`;
}

interface CellProps {
  file: FlatFile;
  fileIdx: number;
  row: number | undefined;
  index: MarkIndex;
  setup: MarkSetup;
  side?: "left" | "right";
  flag?: ReactNode;
}

/** One row's gutter and code, tinted by its mark. Undefined `row` is an empty side. */
export function Cell({ file, fileIdx, row, index, setup, side, flag }: CellProps) {
  if (row === undefined) return <div className="cell empty" />;
  const r = file.rows[row];
  const key = markKey(fileIdx, row);
  const mark = r.kind === "context" ? undefined : index.marks.get(key);
  const amber = !mark && index.amber.has(key);
  return (
    <RowCell
      row={r}
      path={file.dto.path}
      side={side}
      color={mark?.color}
      amber={amber}
      commented={index.commented.has(key)}
      title={amber ? "changed since marked" : markTitle(mark, setup)}
      flag={flag}
    />
  );
}

interface RowCellProps {
  row: RowDto;
  path: string;
  side?: "left" | "right";
  color?: MarkColor;
  amber: boolean;
  commented: boolean;
  title?: string;
  flag?: ReactNode;
}

/** A row as drawn, which renders again only when its own mark, amber or comment changes,
 * not for every edit to the review's state. */
const RowCell = memo(function RowCell(p: RowCellProps) {
  const { row: r, side, color } = p;
  const lineNo = side === "left" ? r.oldLine : side === "right" ? r.newLine : undefined;
  // A line of the new side, which a language server can find in the file.
  const onNew = side !== "left" && r.newLine !== null;
  return (
    <div
      className={`cell${side ? ` ${side}` : ""} kind-${r.kind}${color ? " marked" : ""}${p.amber ? " amber" : ""}${p.commented ? " commented" : ""}`}
      style={color ? ({ "--m": markVar(color) } as React.CSSProperties) : undefined}
      title={p.title}
      data-path={onNew ? p.path : undefined}
      data-line={onNew ? r.newLine : undefined}
    >
      <span className="strip" />
      <span className="flag">{p.flag}</span>
      {side ? (
        <span className="ln">{lineNo ?? ""}</span>
      ) : (
        <>
          <span className="ln">{r.oldLine ?? ""}</span>
          <span className="ln">{r.newLine ?? ""}</span>
        </>
      )}
      <span className="sign">{SIGN[r.kind]}</span>
      <Code row={r} />
    </div>
  );
});
