import type { ContextLine } from "../bindings";
import { Code } from "./Cell";
import type { Region, ViewMode } from "./model";

/** The bar standing in for unchanged code. */
export function MoreBar({ region, onShow }: { region: Region; onShow(): void }) {
  const n = region.to - region.from + 1;
  return (
    <div
      className="more-bar"
      onMouseDown={(e) => {
        e.stopPropagation();
        onShow();
      }}
    >
      ⋯ {n} {n === 1 ? "line" : "lines"}
    </div>
  );
}

function Side({ ln, side, line }: { ln: number; side?: "left" | "right"; line: ContextLine }) {
  return (
    <div className={`cell${side ? ` ${side}` : ""} kind-context shown-context`}>
      <span className="strip" />
      <span className="flag" />
      <span className="ln">{ln}</span>
      <span className="sign" />
      <Code row={line} />
    </div>
  );
}

/** A line of unchanged code shown from a bar. It can't be marked or selected. */
export function ContextRow({
  line,
  old,
  mode,
}: {
  line: ContextLine;
  old: number;
  mode: ViewMode;
}) {
  return (
    <div className={`row-line ${mode}`}>
      {mode === "unified" ? (
        <div className="cell kind-context shown-context">
          <span className="strip" />
          <span className="flag" />
          <span className="ln">{old}</span>
          <span className="ln">{line.line}</span>
          <span className="sign" />
          <Code row={line} />
        </div>
      ) : (
        <>
          <Side ln={old} side="left" line={line} />
          <Side ln={line.line} side="right" line={line} />
        </>
      )}
    </div>
  );
}
