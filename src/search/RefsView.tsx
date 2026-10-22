import { useMemo, useState } from "react";
import type { Hit } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { type CurrentReview, useCurrentReview } from "../review/current";
import { sendToTerminal } from "../terminal/registry";
import { byFile, FileName, HitLine, hitsText } from "./HitLine";
import type { RefsState } from "./store";
import { useHitKeys } from "./useHitKeys";

/** Changed lines of the review's files, as `path:line`. */
function changedLines(review: CurrentReview | undefined): Set<string> {
  const out = new Set<string>();
  for (const f of review?.files ?? []) {
    for (const r of f.rows) {
      if (r.kind === "added" && r.newLine !== null) out.add(`${f.dto.path}:${r.newLine}`);
    }
  }
  return out;
}

/** References to a name: its definition, the uses in the review's files with the lines
 * around them, then the rest folded by file. */
export function RefsView({ ws, refs }: { ws: WorkspaceName; refs?: RefsState }) {
  const review = useCurrentReview(ws);
  const changed = useMemo(() => changedLines(review), [review]);
  const reviewed = useMemo(() => new Set(review?.files.map((f) => f.dto.path)), [review]);
  const [unfolded, setUnfolded] = useState<ReadonlySet<string>>(new Set());
  const hits = refs?.hits ?? [];
  const defs = hits.filter((h) => h.def);
  const inReview = hits.filter((h) => !h.def && reviewed.has(h.path));
  const elsewhere = hits.filter((h) => !h.def && !reviewed.has(h.path));
  const shown = [...defs, ...inReview, ...elsewhere.filter((h) => unfolded.has(h.path))];
  const open = (h: Hit) => openFile(ws, h.path, h.line);
  const keys = useHitKeys(shown, open, () => sendToTerminal(ws, hitsText(hits)));
  const fileCount = new Set(hits.map((h) => h.path)).size;

  if (!refs) return <div className="placeholder">No references</div>;
  const line = (h: Hit, context: boolean) => (
    <HitLine
      key={`${h.path}:${h.line}`}
      hit={h}
      on={shown[keys.at] === h}
      changed={changed.has(`${h.path}:${h.line}`)}
      context={context}
      onOpen={() => {
        keys.setAt(shown.indexOf(h));
        open(h);
      }}
    />
  );
  const lines = (list: Hit[]) => list.map((h) => h.line).join(" :");
  return (
    <div className="sr-body" tabIndex={-1} onKeyDown={keys.onKeyDown}>
      <div className="sr-sum">
        <span className="sr-sym">{refs.name}</span>
        <span className="spacer" />
        {refs.hits ? (
          <>
            <b>{hits.length}</b> in <b>{fileCount}</b> files
          </>
        ) : (
          !refs.error && "Searching…"
        )}
      </div>
      {refs.error && <div className="sr-error">{refs.error}</div>}
      <div className="sr-res" ref={keys.listRef}>
        {defs.length > 0 && <div className="sr-grp">Definition</div>}
        {byFile(defs).map(([path, list]) => (
          <div key={`d:${path}`}>
            <div className="sr-file">
              <span className="sr-car">▾</span>
              <FileName path={path} />
              <span className="sr-at">:{lines(list)}</span>
            </div>
            {list.map((h) => line(h, true))}
          </div>
        ))}
        {inReview.length > 0 && <div className="sr-grp">In this review · {inReview.length}</div>}
        {byFile(inReview).map(([path, list]) => (
          <div key={`r:${path}`}>
            <div className="sr-file">
              <span className="sr-car">▾</span>
              <FileName path={path} />
              <span className="sr-at">:{lines(list)}</span>
            </div>
            {list.map((h) => line(h, true))}
          </div>
        ))}
        {elsewhere.length > 0 && <div className="sr-grp">Elsewhere · {elsewhere.length}</div>}
        {byFile(elsewhere).map(([path, list]) => {
          const open = unfolded.has(path);
          return (
            <div key={`e:${path}`}>
              <button
                type="button"
                className="sr-file"
                onClick={() => {
                  const next = new Set(unfolded);
                  if (open) next.delete(path);
                  else next.add(path);
                  setUnfolded(next);
                }}
              >
                <span className="sr-car">{open ? "▾" : "▸"}</span>
                <FileName path={path} />
                <span className="sr-cnt">{list.length}</span>
              </button>
              {open && list.map((h) => line(h, false))}
            </div>
          );
        })}
      </div>
      <div className="sr-foot">
        <span>
          <kbd>↑↓</kbd> move
        </span>
        <span>
          <kbd>↵</kbd> open
        </span>
        <span>
          <kbd>⌥↵</kbd> send to terminal
        </span>
      </div>
    </div>
  );
}
