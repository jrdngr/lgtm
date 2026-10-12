import { HIGHLIGHT_NAMES, type MarkDto, type RowDto } from "../bindings";
import { type FlatFile, markKey, SIGN } from "./model";
import type { MarkIndex } from "./state";
import "./syntax.css";

const hlClass = HIGHLIGHT_NAMES.map((n) => `hl-${n.replace(".", "-")}`);

function Code({ row }: { row: RowDto }) {
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

function markTitle(m: MarkDto | undefined) {
  if (!m) return undefined;
  const when = m.createdAt === null ? "" : ` · ${new Date(m.createdAt).toLocaleString()}`;
  return `${m.state} in ${m.review}${when}`;
}

interface CellProps {
  file: FlatFile;
  fileIdx: number;
  row: number | undefined;
  index: MarkIndex;
  marker?: { name: string; onClick: () => void };
  side?: "left" | "right";
}

/** One row's gutter and code, with its mark as classes. Undefined `row` is an empty side. */
export function Cell({ file, fileIdx, row, index, marker, side }: CellProps) {
  if (row === undefined) return <div className="cell empty" />;
  const r = file.rows[row];
  const key = markKey(fileIdx, row);
  const mark = r.kind === "context" ? undefined : index.marks.get(key);
  const checked = r.kind !== "context" && index.checks[fileIdx] === "checked";
  const state = mark?.state ?? (checked ? "reviewed" : undefined);
  const amber = !state && index.amber.has(key);
  const commented = index.commented.has(key);
  const hot = index.hot.get(key);
  const lineNo = side === "left" ? r.oldLine : side === "right" ? r.newLine : undefined;
  return (
    <div
      className={`cell kind-${r.kind}${state ? ` mark-${state}` : ""}${amber ? " amber" : ""}${commented ? " commented" : ""}`}
      title={amber ? "changed since reviewed" : markTitle(mark)}
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
