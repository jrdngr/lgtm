import { HIGHLIGHT_NAMES, type MarkDto, type RowDto } from "../bindings";
import { type MarkSetup, markVar } from "./marks";
import { type FlatFile, markKey, SIGN } from "./model";
import type { MarkIndex } from "./state";
import "./syntax.css";

const hlClass = HIGHLIGHT_NAMES.map((n) => `hl-${n.replace(".", "-")}`);

/** A line's code with its highlighting. */
export function Code({ row }: { row: Pick<RowDto, "text" | "hl"> }) {
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
}

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
  marker?: { name: string; onClick: () => void };
  side?: "left" | "right";
}

/** One row's gutter and code, tinted by its mark. A checked file's unmarked rows show the
 * first mark color. Undefined `row` is an empty side. */
export function Cell({ file, fileIdx, row, index, setup, marker, side }: CellProps) {
  if (row === undefined) return <div className="cell empty" />;
  const r = file.rows[row];
  const key = markKey(fileIdx, row);
  const mark = r.kind === "context" ? undefined : index.marks.get(key);
  const checked = r.kind !== "context" && index.checks[fileIdx] === "checked";
  const color = mark?.color ?? (checked ? setup.primary : undefined);
  const amber = !color && index.amber.has(key);
  const commented = index.commented.has(key);
  const hot = index.hot.get(key);
  const lineNo = side === "left" ? r.oldLine : side === "right" ? r.newLine : undefined;
  return (
    <div
      className={`cell${side ? ` ${side}` : ""} kind-${r.kind}${color ? " marked" : ""}${amber ? " amber" : ""}${commented ? " commented" : ""}`}
      style={color ? ({ "--m": markVar(color) } as React.CSSProperties) : undefined}
      title={amber ? "changed since marked" : markTitle(mark, setup)}
    >
      <span
        className={`risk${hot?.level ? ` risk-${hot.level}` : ""}`}
        title={hot ? `${hot.level}: ${hot.title}` : undefined}
      />
      <span className="strip" />
      <span
        className={`fn-marker${marker ? " on" : ""}`}
        title={marker ? `select ${marker.name}` : undefined}
        onMouseDown={(e) => {
          if (!marker) return;
          e.stopPropagation();
          marker.onClick();
        }}
      >
        {marker ? "ƒ" : ""}
      </span>
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
}
