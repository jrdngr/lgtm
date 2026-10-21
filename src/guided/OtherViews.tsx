import { useState } from "react";
import type { ChunkRisk, CommentDto, GuidedDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";
import type { Comments } from "../review/useComments";
import type { CommentFolds } from "../review/useSavedView";
import { ChunkCode } from "./ChunkCode";
import { commentsAfter, ringedRows, startComment, useLineSel } from "./ChunkView";
import { type ExcludedState, REASON_NAMES, RISK_NAMES, RISKS, threadSegment } from "./model";
import { setGuided, setPick } from "./store";

/** Code the agent left out, with its reason, and Promote to make it a chunk at a risk the
 * person picks. */
export function ExcludedView({
  workspace,
  review,
  e,
  setError,
}: {
  workspace: WorkspaceName;
  review: CurrentReview;
  e: ExcludedState;
  setError(message: string): void;
}) {
  const [picking, setPicking] = useState(false);
  function promote(risk: ChunkRisk) {
    setPicking(false);
    unwrap(commands.promoteExcluded(review.id, e.index, risk)).then(
      (g) => {
        setGuided(review.id, g);
        const made = g.chunks[g.chunks.length - 1];
        if (made) setPick(workspace, { kind: "chunk", id: made.id });
      },
      (err) => setError(String(err)),
    );
  }
  function onKeyDown(k: React.KeyboardEvent) {
    if (k.metaKey || k.ctrlKey || k.altKey) return;
    const at = Number(k.key);
    if (k.key === "p") setPicking(!picking);
    else if (k.key === "Escape") setPicking(false);
    else if (picking && at >= 1 && at <= RISKS.length) promote(RISKS[at - 1]);
    else return;
    k.preventDefault();
  }
  return (
    <div className="gd-view" tabIndex={-1} onKeyDown={onKeyDown}>
      <div className="gd-head">
        <span className="gd-risk excluded">Excluded</span>
        <span className="gd-name mono">{e.part.path}</span>
        <span className="spacer" />
        <span className="gd-pop-anchor">
          <button type="button" className="gd-btn" onClick={() => setPicking(!picking)}>
            Promote <kbd>p</kbd>
          </button>
          {picking && (
            <div className="gd-pop right">
              {RISKS.map((risk, i) => (
                <button key={risk} type="button" className="gd-mi" onClick={() => promote(risk)}>
                  <span className={`gd-dot risk-${risk}`} />
                  {RISK_NAMES[risk]}
                  <span className="spacer" />
                  <kbd>{i + 1}</kbd>
                </button>
              ))}
            </div>
          )}
        </span>
      </div>
      <div className="gd-writeup">
        {REASON_NAMES[e.part.reason]}
        {e.part.why && `: ${e.part.why}`}
      </div>
      <ChunkCode files={review.files} segments={e.segments} />
    </div>
  );
}

/** The ADRs and dev logs the change bears on, with the chunks each touches. */
export function ContextView({ g, workspace: ws }: { g: GuidedDto; workspace: WorkspaceName }) {
  return (
    <div className="gd-view" tabIndex={-1}>
      <div className="gd-head">
        <span className="gd-name">Context</span>
      </div>
      {g.context.map((d) => {
        const chunks = g.chunks.filter((c) => c.docs.includes(d.path));
        return (
          <div key={d.path} className="gd-card">
            <div className="gd-card-head">
              <b>{d.kind === "adr" ? "ADR" : d.kind === "devlog" ? "Dev log" : "Doc"}</b>
              <span>{d.title}</span>
              <span className="mono gd-path">{d.path}</span>
              <span className="spacer" />
              <button type="button" className="gd-link" onClick={() => openFile(ws, d.path)}>
                Open
              </button>
            </div>
            <Markdown>{d.summary}</Markdown>
            {chunks.length > 0 && (
              <div className="gd-chips">
                {chunks.map((c) => (
                  <button
                    key={c.id}
                    type="button"
                    className="gd-tag"
                    onClick={() => setPick(ws, { kind: "chunk", id: c.id })}
                  >
                    {c.title}
                  </button>
                ))}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

/** A PR thread on code below Low: the thread, then its code. */
export function CommentedView({
  review,
  comment,
  c,
  folds,
}: {
  review: CurrentReview;
  comment: CommentDto;
  c: Comments;
  folds: CommentFolds;
}) {
  const segments = threadSegment(review.files, comment);
  const ringed = ringedRows(c, segments, new Set([comment.id]));
  const { sel, setSel, onRow } = useLineSel(review, segments);
  function onKeyDown(k: React.KeyboardEvent) {
    if (k.metaKey || k.ctrlKey || k.altKey || !sel) return;
    if (k.target instanceof HTMLTextAreaElement) return;
    if (k.key !== "c" && !(k.key === "C" && c.canDraft)) return;
    k.preventDefault();
    startComment(review, c, sel, k.key === "C");
    setSel(undefined);
  }
  return (
    <div className="gd-view" tabIndex={-1} onKeyDown={onKeyDown}>
      <div className="gd-head">
        <span className="gd-risk commented">Commented</span>
        <span className="gd-name mono">{comment.path}</span>
      </div>
      <ChunkCode
        files={review.files}
        segments={segments}
        ringed={ringed}
        sel={sel}
        onRow={onRow}
        after={commentsAfter(c, folds, segments)}
      />
    </div>
  );
}
