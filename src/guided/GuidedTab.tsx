import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReviewDto } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { usePr } from "../github/pr";
import { commands, unwrap } from "../ipc";
import { type CurrentReview, useCurrentReview } from "../review/current";
import { useMarkSetup } from "../review/marks";
import { useComments } from "../review/useComments";
import type { CommentFolds } from "../review/useSavedView";
import { useSendText } from "../review/useSendText";
import { PipeButton } from "../terminal/PipeButton";
import { keepSubmit } from "../terminal/registry";
import { Empty } from "../ui/SectionHeader";
import { ChunkView } from "./ChunkView";
import type { GuidedView } from "./model";
import { CommentedView, ContextView, ExcludedView } from "./OtherViews";
import { type Pick, setPick, usePick } from "./store";
import { useGuidedView } from "./useGuidedView";
import "../review/review.css";
import "./guided.css";

/** Comment folds kept while the tab is open. */
function useFolds(): CommentFolds {
  const [folds, setFolds] = useState<ReadonlyMap<number, [boolean, string]>>(new Map());
  return useMemo(
    () => ({
      get(id, state) {
        const f = folds.get(id);
        return f && f[1] === state ? f[0] : undefined;
      },
      set(id, folded, state) {
        setFolds((m) => new Map(m).set(id, [folded, state]));
      },
    }),
    [folds],
  );
}

/** What comments need of a review, from the current one. */
const reviewOf = (r: CurrentReview | undefined) =>
  r && ({ id: r.id, state: { comments: r.comments, threads: r.threads } } as unknown as ReviewDto);

/** Whether the guided review still has what `p` names. */
function pickExists(p: Pick, view: GuidedView, review: CurrentReview): boolean {
  if (p.kind === "context") return true;
  if (p.kind === "chunk") return view.states.some((s) => s.chunk.id === p.id);
  if (p.kind === "commented") return review.comments.some((c) => c.id === p.id);
  return [...view.excluded, ...view.tests].some((e) => e.index === p.index);
}

/** The guided review's picked chunk, thread, excluded part or context docs. */
export function GuidedTab({ api }: IDockviewPanelProps) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const setup = useMarkSetup();
  const pr = usePr(workspace);
  const { g, error: loadError, view } = useGuidedView(review);
  const picked = usePick(workspace);
  const [error, setError] = useState<string>();
  const root = useRef<HTMLDivElement>(null);
  const focus = useCallback(
    () => root.current?.querySelector<HTMLElement>(".gd-view")?.focus({ preventScroll: true }),
    [],
  );
  const { sendText, picker } = useSendText(workspace, setError, focus);
  const background = useSendText(workspace, setError, focus, "background");
  const dto = useMemo(() => reviewOf(review), [review]);
  const c = useComments({
    review: dto,
    files: review?.files ?? [],
    moves: undefined,
    onState: (state, id) => review?.apply(state, id),
    onError: setError,
    workspace,
    send: sendText,
    onDone: focus,
    canDraft: !!pr,
  });
  const folds = useFolds();

  // Without a pick, or once the picked one is gone, the first chunk still to review shows, or
  // else the first. It becomes the pick, so the sidebar marks it.
  const first = view?.states.find((s) => !s.done) ?? view?.states[0];
  const fallback: Pick | undefined = first && { kind: "chunk", id: first.chunk.id };
  const valid = !!picked && !!view && !!review && pickExists(picked, view, review);
  const pick = valid ? picked : fallback;
  const pickKey = JSON.stringify(pick);
  // biome-ignore lint/correctness/useExhaustiveDependencies: follows the pick shown
  useEffect(() => {
    if (view && pick && !valid) setPick(workspace, pick);
  }, [pickKey, valid, view]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: keys go to the view when it changes
  useEffect(() => {
    if (api.isActive) focus();
  }, [pickKey]);
  useEffect(() => {
    const sub = api.onDidActiveChange(({ isActive }) => isActive && focus());
    return () => sub.dispose();
  }, [api, focus]);

  function generate() {
    if (!review) return;
    keepSubmit(unwrap(commands.guidedPrompt(review.id)))
      .then((text) => background.sendText(text))
      .catch((e) => setError(String(e)));
  }

  const problem = error ?? loadError;
  let body: React.ReactNode;
  if (!review) body = <Empty>No review open</Empty>;
  else if (g === null)
    body = (
      <div className="placeholder">
        <PipeButton background onClick={generate}>
          Generate
        </PipeButton>
      </div>
    );
  else if (!view || !g) body = <Empty>Loading…</Empty>;
  else if (pick?.kind === "context") body = <ContextView g={g} workspace={workspace} />;
  else if (pick?.kind === "excluded") {
    const e = [...view.excluded, ...view.tests].find((x) => x.index === pick.index);
    body = e ? (
      <ExcludedView
        key={e.index}
        workspace={workspace}
        review={review}
        setup={setup}
        e={e}
        c={c}
        folds={folds}
        send={sendText}
        setError={setError}
      />
    ) : (
      <Empty>Gone</Empty>
    );
  } else if (pick?.kind === "commented") {
    const t = review.comments.find((x) => x.id === pick.id);
    body = t ? (
      <CommentedView
        key={t.id}
        workspace={workspace}
        review={review}
        setup={setup}
        comment={t}
        c={c}
        folds={folds}
        send={sendText}
        setError={setError}
      />
    ) : (
      <Empty>Gone</Empty>
    );
  } else {
    const s = view.states.find((x) => x.chunk.id === pick?.id);
    body = s ? (
      <ChunkView
        review={review}
        setup={setup}
        view={view}
        s={s}
        c={c}
        folds={folds}
        send={sendText}
        docs={g.context.filter((d) => s.chunk.docs.includes(d.path))}
        setError={setError}
      />
    ) : (
      <Empty>No chunks</Empty>
    );
  }

  return (
    <div className="guided-tab" ref={root}>
      {problem && (
        <div className="review-error">
          {problem}
          <button type="button" className="gd-link" onClick={() => setError(undefined)}>
            ×
          </button>
        </div>
      )}
      {body}
      {picker}
    </div>
  );
}
