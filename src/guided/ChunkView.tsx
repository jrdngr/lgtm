import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import type { ContextDocDto, PromptDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import { remembered } from "../core/remembered";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";
import { CommentBlock } from "../review/CommentBlock";
import type { CurrentReview } from "../review/current";
import { Diffstat } from "../review/Diffstat";
import type { MarkSetup } from "../review/marks";
import { markKey, NEW_COMMENT } from "../review/model";
import type { Comments } from "../review/useComments";
import type { CommentFolds } from "../review/useSavedView";
import { PipeButton } from "../terminal/PipeButton";
import { submitNextSend } from "../terminal/registry";
import { ago } from "../ui/time";
import { askText, changeText, giveVerdict } from "./actions";
import { ChunkCode, RelatedCode } from "./ChunkCode";
import { ChunkSummary } from "./ChunkSummary";
import { showChunk } from "./focus";
import {
  type ChunkState,
  type GuidedView,
  lineDiff,
  nextChunk,
  RISK_NAMES,
  type Segment,
  stepParts,
  withWholeFiles,
} from "./model";
import { Superseded } from "./Superseded";
import { setPick } from "./store";
import { useGuidedLines } from "./useGuidedLines";

/** The summary and chips under a chunk's title are folded away, in every chunk. */
const topFolded = remembered("lgtm:guidedTopFolded", false, [false, true]);

const DOC_NAMES = { adr: "ADR", devlog: "Dev log", other: "Doc" };

/** Comment blocks under the last row each comment covers in `segments`: its threads, notes
 * and the draft being written. */
export function commentsAfter(
  c: Comments,
  folds: CommentFolds,
  segments: Segment[],
): (file: number, row: number) => ReactNode {
  const at = new Map<string, number[]>();
  const shown = new Map(segments.map((g) => [g.file, new Set<number>()]));
  for (const g of segments) for (const r of g.rows) shown.get(g.file)?.add(r);
  const place = (file: number, start: number, end: number, id: number) => {
    const rows = shown.get(file);
    if (!rows) return;
    for (let r = end; r >= start; r--) {
      if (!rows.has(r)) continue;
      const key = markKey(file, r);
      at.set(key, [...(at.get(key) ?? []), id]);
      return;
    }
  };
  for (const t of c.comments) if (t.rows) place(t.file, t.rows.start, t.rows.end, t.id);
  const d = c.draft;
  if (d?.rows.length) place(d.file, d.rows[0], d.rows[d.rows.length - 1], NEW_COMMENT);
  return (file, row) => {
    const ids = at.get(markKey(file, row));
    if (!ids) return null;
    return (
      <div className="gd-inline">
        {ids.map((id) => (
          <CommentBlock
            key={id}
            id={id}
            comment={c.comments.find((x) => x.id === id)}
            c={c}
            folds={folds}
          />
        ))}
      </div>
    );
  };
}

interface Props {
  review: CurrentReview;
  setup: MarkSetup;
  view: GuidedView;
  s: ChunkState;
  c: Comments;
  folds: CommentFolds;
  send(text: string): void;
  docs: ContextDocDto[];
  setError(message: string): void;
}

/** One chunk: the agent's summary, its docs and tests, and its code with its PR threads, with the
 * verdicts that move on to the next chunk. */
export function ChunkView({ review, setup, view, s, c, folds, send, docs, setError }: Props) {
  const workspace = c.workspace;
  const { chunk } = s;
  const [steps, setSteps] = useState(false);
  const [step, setStep] = useState(0);
  const folded = topFolded.use();
  /** Files shown whole in Steps, by review file. */
  const [whole, setWhole] = useState<ReadonlySet<number>>(new Set());
  const [since, setSince] = useState(s.changed);
  const [noting, setNoting] = useState<string>();
  const [prompts, setPrompts] = useState<PromptDto[]>();
  const [promptsOpen, setPromptsOpen] = useState(false);
  const current = docs.filter((d) => !d.superseded);
  const lit = steps ? chunk.steps[step]?.ranges : undefined;
  const { segments, related } = useMemo(() => {
    const parts = stepParts(review.files, s.segments, chunk.related, lit);
    return steps
      ? { ...parts, segments: withWholeFiles(review.files, parts.segments, whole) }
      : parts;
  }, [review.files, s.segments, chunk.related, lit, whole, steps]);
  const toggleWhole = (file: number) =>
    setWhole((w) => {
      const next = new Set(w);
      if (!next.delete(file)) next.add(file);
      return next;
    });
  const lines = useGuidedLines({ workspace, review, setup, segments, c, send, setError });
  const after = commentsAfter(c, folds, s.segments);
  const stepCode = useRef<HTMLDivElement>(null);

  // A picked step brings its lines into view.
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs when the step changes
  useEffect(() => {
    if (!steps) return;
    const first = stepCode.current?.querySelector(".gd-ln.lit");
    first?.scrollIntoView({ block: "center", behavior: "smooth" });
    stepCode.current?.parentElement
      ?.querySelector(".gd-step.on")
      ?.scrollIntoView({ block: "nearest" });
  }, [steps, step]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: a new chunk starts fresh
  useEffect(() => {
    setSteps(false);
    setStep(0);
    setWhole(new Set());
    setSince(s.changed);
    setNoting(undefined);
    lines.clear();
    setPromptsOpen(false);
    const paths = [...new Set(chunk.ranges.map((r) => r.path))];
    const added = s.segments.flatMap((g) =>
      g.rows.flatMap((r) => {
        const row = review.files[g.file].rows[r];
        return row.kind === "added" ? [row.text] : [];
      }),
    );
    let live = true;
    unwrap(commands.chunkPrompts(paths, added))
      .then((p) => live && setPrompts(p))
      .catch(() => live && setPrompts([]));
    return () => {
      live = false;
    };
  }, [chunk.id]);

  const advance = () => {
    const next = nextChunk(view.states, chunk.id);
    if (next) setPick(workspace, { kind: "chunk", id: next });
  };
  const fail = (e: unknown) => setError(String(e));
  const approve = () => {
    giveVerdict(review, setup, [s], "approved").then(advance, fail);
  };
  const ask = () => send(askText(s));
  const change = () => setNoting(noting ?? "");
  function sendChange() {
    const note = noting?.trim();
    if (!note) return;
    submitNextSend();
    send(changeText(s, note));
    setNoting(undefined);
    giveVerdict(review, setup, [s], "change", note).then(advance, fail);
  }
  function onKeyDown(e: React.KeyboardEvent) {
    if (e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement) return;
    if (lines.onKey(e)) {
      e.preventDefault();
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const keys: Record<string, () => void> = {
      a: approve,
      c: change,
      "?": ask,
    };
    if (steps) {
      keys.ArrowLeft = () => setStep((n) => Math.max(n - 1, 0));
      keys.ArrowRight = () => setStep((n) => Math.min(n + 1, chunk.steps.length - 1));
    }
    const act = keys[e.key];
    if (!act) return;
    e.preventDefault();
    act();
  }

  const code = (
    <>
      {since && s.verdict ? (
        <SinceDiff before={s.verdict.text} after={s.text} />
      ) : (
        <ChunkCode
          files={review.files}
          segments={segments}
          lit={lit}
          lines={lines}
          after={after}
          expanded={whole}
          onExpand={steps ? toggleWhole : undefined}
        />
      )}
      {related.map((r) => (
        <RelatedCode key={`${r.path}:${r.start}`} workspace={workspace} range={r} lit={lit} />
      ))}
    </>
  );

  return (
    <div className={`gd-view${steps ? " stepping" : ""}`} tabIndex={-1} onKeyDown={onKeyDown}>
      <div className="gd-top">
        <div className="gd-head">
          <button
            type="button"
            className="gd-fold"
            aria-label={folded ? "Unfold" : "Fold"}
            aria-expanded={!folded}
            onClick={() => topFolded.set(!folded)}
          >
            {folded ? "▸" : "▾"}
          </button>
          <span className={`gd-risk risk-${chunk.risk}`}>{RISK_NAMES[chunk.risk]}</span>
          <span className="gd-name">{chunk.title}</span>
          <Diffstat stat={s.stat} />
          {s.changed && <span className="pill amber">Changed</span>}
          {chunk.tags.map((t) => (
            <span key={t} className="gd-tag">
              {t}
            </span>
          ))}
          <span className="spacer" />
          {prompts && prompts.length > 0 && (
            <span className="gd-pop-anchor">
              <button type="button" className="gd-btn" onClick={() => setPromptsOpen(!promptsOpen)}>
                Prompts {prompts.length}
              </button>
              {promptsOpen && (
                <div className="gd-pop">
                  {prompts.map((p) => (
                    <div key={`${p.session}:${p.prompt}`} className="gd-prompt">
                      <span>{p.prompt}</span>
                      {p.at !== null && <span className="gd-when">{ago(p.at)}</span>}
                    </div>
                  ))}
                </div>
              )}
            </span>
          )}
          <button
            type="button"
            className="gd-btn"
            onClick={() =>
              showChunk(workspace, {
                title: chunk.title,
                paths: [...new Set(chunk.ranges.map((r) => r.path))],
              })
            }
          >
            Architecture
          </button>
          {s.changed && s.verdict && (
            <span className="gd-seg">
              <button
                type="button"
                aria-pressed={since}
                onClick={() => {
                  setSince(true);
                  setSteps(false);
                }}
              >
                Since you reviewed
              </button>
              <button type="button" aria-pressed={!since} onClick={() => setSince(false)}>
                Whole chunk
              </button>
            </span>
          )}
          {chunk.steps.length > 0 && (
            <button
              type="button"
              className="gd-btn"
              aria-pressed={steps}
              onClick={() => {
                setSteps(!steps);
                setSince(false);
              }}
            >
              Steps
            </button>
          )}
          <span className="gd-sep" />
          <button type="button" className="gd-btn" onClick={ask}>
            Ask <kbd>?</kbd>
          </button>
          <button type="button" className="gd-btn" onClick={change}>
            Change <kbd>c</kbd>
          </button>
          <button type="button" className="gd-btn approve" onClick={approve}>
            Approve <kbd>a</kbd>
          </button>
        </div>

        {noting !== undefined && (
          <div className="gd-note">
            <textarea
              // biome-ignore lint/a11y/noAutofocus: Change opens it to type in
              autoFocus
              value={noting}
              onChange={(e) => setNoting(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || !e.shiftKey)) {
                  e.preventDefault();
                  sendChange();
                } else if (e.key === "Escape") setNoting(undefined);
              }}
            />
            <div className="gd-note-actions">
              <button type="button" className="gd-btn" onClick={() => setNoting(undefined)}>
                Cancel
              </button>
              <PipeButton onClick={sendChange} disabled={!noting.trim()}>
                Change
              </PipeButton>
            </div>
          </div>
        )}

        {s.verdict?.verdict === "change" && s.verdict.note && noting === undefined && (
          <div className="gd-asked">
            <span className="gd-asked-label">Your change</span>
            <div>{s.verdict.note}</div>
          </div>
        )}

        {!steps && !folded && (
          <>
            <ChunkSummary summary={chunk.summary} />
            {(docs.length > 0 || chunk.tests) && (
              <div className="gd-chips">
                {current.map((d) => (
                  <span key={d.path} className="gd-chip">
                    <b>{DOC_NAMES[d.kind]}</b>
                    <span>{d.title}</span>
                    <button
                      type="button"
                      className="gd-link"
                      onClick={() => openFile(workspace, d.path)}
                    >
                      Open
                    </button>
                  </span>
                ))}
                <Superseded workspace={workspace} docs={docs.filter((d) => d.superseded)} />
                {chunk.tests && (
                  <span className="gd-chip">
                    <b>Tests</b>
                    {chunk.tests.passed > 0 && (
                      <span className="gd-pass">{chunk.tests.passed} pass</span>
                    )}
                    {chunk.tests.failed > 0 && (
                      <span className="gd-fail">{chunk.tests.failed} fail</span>
                    )}
                    {chunk.tests.uncovered.length > 0 && (
                      <span className="gd-uncovered">
                        {chunk.tests.uncovered.reduce((n, r) => n + r.end - r.start + 1, 0)}{" "}
                        uncovered
                      </span>
                    )}
                  </span>
                )}
              </div>
            )}
          </>
        )}
        {lines.picked.length > 0 && (
          <div className="gd-selbar">
            <span>
              {lines.picked.length} {lines.picked.length === 1 ? "line" : "lines"}
            </span>
            {lines.canComment && (
              <button type="button" className="gd-btn" onClick={() => lines.comment(false)}>
                Comment <kbd>c</kbd>
              </button>
            )}
            {lines.canComment && c.canDraft && (
              <button type="button" className="gd-btn" onClick={() => lines.comment(true)}>
                Draft <kbd>⇧C</kbd>
              </button>
            )}
          </div>
        )}
      </div>

      {steps ? (
        <div className="gd-steps">
          <div className="gd-step-list">
            {chunk.steps.map((st, i) => (
              <button
                // biome-ignore lint/suspicious/noArrayIndexKey: steps are in order and never move
                key={i}
                type="button"
                className={`gd-step${i === step ? " on" : i < step ? " past" : ""}`}
                onClick={() => setStep(i)}
              >
                <span className="gd-num">{i + 1}</span>
                <span>
                  <Markdown>{st.text}</Markdown>
                </span>
              </button>
            ))}
            <div className="gd-step-nav">
              <button
                type="button"
                className="gd-btn"
                disabled={step === 0}
                onClick={() => setStep(step - 1)}
              >
                ←
              </button>
              <span>
                {step + 1} / {chunk.steps.length}
              </span>
              <button
                type="button"
                className="gd-btn"
                disabled={step >= chunk.steps.length - 1}
                onClick={() => setStep(step + 1)}
              >
                →
              </button>
            </div>
          </div>
          <div className="gd-step-code" ref={stepCode}>
            {code}
          </div>
        </div>
      ) : (
        code
      )}
    </div>
  );
}

/** What changed in a chunk's code since its verdict, as a diff of the two diffs. */
function SinceDiff({ before, after }: { before: string; after: string }) {
  const lines = useMemo(() => lineDiff(before, after), [before, after]);
  return (
    <div className="gd-code">
      {lines.map((l, i) => (
        <div
          // biome-ignore lint/suspicious/noArrayIndexKey: a diff's lines are in order
          key={i}
          className={`gd-ln since-${l.kind}`}
        >
          <span className="n" />
          <span className="s">{l.kind === "added" ? "+" : l.kind === "removed" ? "−" : ""}</span>
          <span className="code">{l.text}</span>
        </div>
      ))}
    </div>
  );
}
