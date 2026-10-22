import { useMemo, useState } from "react";
import type { ChunkRisk, CommentDto, GuidedDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";
import type { MarkSetup } from "../review/marks";
import type { Comments } from "../review/useComments";
import type { CommentFolds } from "../review/useSavedView";
import { ChunkCode } from "./ChunkCode";
import { commentsAfter } from "./ChunkView";
import { type ExcludedState, REASON_NAMES, RISK_NAMES, RISKS, threadSegment } from "./model";
import { Superseded } from "./Superseded";
import { setGuided, setPick } from "./store";
import { useGuidedLines } from "./useGuidedLines";

/** Code the agent left out, with its reason, and Promote to make it a chunk at a risk the
 * person picks. */
export function ExcludedView({
  workspace,
  review,
  setup,
  e,
  c,
  folds,
  send,
  setError,
}: {
  workspace: WorkspaceName;
  review: CurrentReview;
  setup: MarkSetup;
  e: ExcludedState;
  c: Comments;
  folds: CommentFolds;
  send(text: string): void;
  setError(message: string): void;
}) {
  const [picking, setPicking] = useState(false);
  const lines = useGuidedLines({
    workspace,
    review,
    setup,
    segments: e.segments,
    c,
    send,
    setError,
  });
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
    if (k.target instanceof HTMLTextAreaElement) return;
    const at = Number(k.key);
    if (!picking && lines.onKey(k)) {
      k.preventDefault();
      return;
    }
    if (k.metaKey || k.ctrlKey || k.altKey) return;
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
      <ChunkCode
        files={review.files}
        segments={e.segments}
        lines={lines}
        after={commentsAfter(c, folds, e.segments)}
      />
    </div>
  );
}

/** The ADRs and dev logs the change bears on, with the chunks each touches. */
export function ContextView({ g, workspace: ws }: { g: GuidedDto; workspace: WorkspaceName }) {
  return (
    <div className="gd-view" tabIndex={-1}>
      <div className="gd-head">
        <span className="gd-name">Context</span>
        <span className="spacer" />
        <Superseded workspace={ws} docs={g.context.filter((d) => d.superseded)} right />
      </div>
      {g.context
        .filter((d) => !d.superseded)
        .map((d) => {
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
  workspace,
  review,
  setup,
  comment,
  c,
  folds,
  send,
  setError,
}: {
  workspace: WorkspaceName;
  review: CurrentReview;
  setup: MarkSetup;
  comment: CommentDto;
  c: Comments;
  folds: CommentFolds;
  send(text: string): void;
  setError(message: string): void;
}) {
  const segments = useMemo(() => threadSegment(review.files, comment), [review.files, comment]);
  const lines = useGuidedLines({ workspace, review, setup, segments, c, send, setError });
  function onKeyDown(k: React.KeyboardEvent) {
    if (k.target instanceof HTMLTextAreaElement) return;
    if (lines.onKey(k)) k.preventDefault();
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
        lines={lines}
        after={commentsAfter(c, folds, segments)}
      />
    </div>
  );
}
