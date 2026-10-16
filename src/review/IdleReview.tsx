import { useEffect, useRef, useState } from "react";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import {
  type CurrentReview,
  currentReview,
  isCurrent,
  publishReview,
  retractReview,
  useCurrentReview,
} from "./current";
import { inReviewPane, isIdle, markIdle } from "./idle";
import { useMarkSetup } from "./marks";
import { startRevset, workspaceRevset } from "./revset";
import { publishTarget, type ReviewTarget, retractTarget, targetOf } from "./target";
import { useAgentRequests } from "./useAgentRequests";
import { useReview } from "./useReview";
import { useReviewState } from "./useReviewState";

const ignore = () => {};

/** Keeps the workspace's review loaded while no review pane shows it. Runs while the dock has
 * no review pane, and while a new pane loads and its idle review still stands in for it. */
export function WorkspaceReview({
  workspace,
  hasPane,
}: {
  workspace: WorkspaceName;
  hasPane: boolean;
}) {
  const current = useCurrentReview(workspace);
  return !hasPane || isIdle(current) ? <IdleReview workspace={workspace} /> : null;
}

/** Loads the workspace's revset for the header picker and the side panels. Whatever needs
 * the code on screen opens a review pane and is done there. */
function IdleReview({ workspace }: { workspace: WorkspaceName }) {
  const { loaded, loading, open, setState } = useReview(workspace);
  const [revset, setRevset] = useState(() => workspaceRevset(workspace) ?? "@");
  const review = loaded?.review;
  const setup = useMarkSetup();
  const { files, index, rowState, progress } = useReviewState(review, setup.done);
  const published = useRef<CurrentReview>(undefined);
  const target = useRef<ReviewTarget>(undefined);

  async function load(next: string): Promise<string | undefined> {
    const r = await open(next);
    if (typeof r === "string") return r;
    setRevset(next);
    return undefined;
  }
  const loadRef = useRef(load);
  loadRef.current = load;

  // biome-ignore lint/correctness/useExhaustiveDependencies: load once
  useEffect(() => {
    startRevset(workspace).then(load);
  }, []);

  const setStateRef = useRef(setState);
  setStateRef.current = setState;

  // A pane that took over keeps what it published.
  useEffect(() => {
    if (targetOf(workspace) && !isIdle(targetOf(workspace))) return;
    target.current = markIdle({
      revset,
      changes: review?.changes ?? [],
      loading,
      load: (r) => loadRef.current(r),
      focus: ignore,
    });
    publishTarget(workspace, target.current);
  }, [workspace, revset, review, loading]);

  useEffect(() => {
    if (!review || (currentReview(workspace) && !isIdle(currentReview(workspace)))) return;
    const inPane = (f: (r: CurrentReview) => void) => inReviewPane(workspace, f);
    published.current = markIdle<CurrentReview>({
      id: review.id,
      revset: review.revset,
      to: review.to,
      files,
      progress,
      rowState,
      done: setup.done,
      checks: index.checks,
      filter: "all",
      comments: review.state.comments,
      notes: review.state.notes,
      threads: review.state.threads,
      apply: (state, review) => setStateRef.current(state, review),
      jump: (file) => inPane((r) => r.jump(file)),
      jumpToComment: (id) => inPane((r) => r.jumpToComment(id)),
      jumpToRow: (file, row) => inPane((r) => r.jumpToRow(file, row)),
      show: (path, line) => {
        if (files.some((f) => f.dto.path === path || f.dto.oldPath === path)) {
          inPane((r) => r.show(path, line));
        } else openFile(workspace, path, line ?? undefined);
      },
      send: (text, onSent, report) => inPane((r) => r.send(text, onSent, report)),
      runWalkthrough: () => inPane((r) => r.runWalkthrough()),
      refetchPr: () => inPane((r) => r.refetchPr()),
      pullPr: (quiet) => inPane((r) => r.pullPr(quiet)),
      async commentOn(file, rows, body, snippet, draft = false) {
        const refs = rows.map((row) => ({ file, row }));
        const r = await commands.addComment(review.id, refs, body, snippet, draft);
        if (r.status === "error") return false;
        setStateRef.current(r.data);
        return true;
      },
    });
    publishReview(workspace, published.current);
    // Agents in the workspace's terminals act on this review.
    commands.focusReview(workspace, review.id);
  }, [workspace, review, files, progress, rowState, setup.done, index]);

  useEffect(
    () => () => {
      retractReview(workspace, published.current);
      retractTarget(workspace, target.current);
    },
    [workspace],
  );

  useAgentRequests(workspace, {
    isCurrent: () => isCurrent(workspace, published.current),
    show: (path, line) => published.current?.show(path, line),
    load,
  });

  return null;
}
