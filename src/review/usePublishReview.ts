import type { DockviewPanelApi } from "dockview-react";
import { useEffect, useRef } from "react";
import type { CheckState, ReviewDto } from "../bindings";
import { commands } from "../ipc";
import { inDock } from "../workbench/docks";
import type { WorkspaceName } from "../workspaces/context";
import {
  type CurrentReview,
  currentReview,
  isCurrent,
  onReviewChange,
  publishReview,
  retractReview,
} from "./current";
import { isIdle } from "./idle";
import type { FlatFile } from "./model";
import type { Filter, Progress, RowStateFn } from "./state";

interface Data {
  files: FlatFile[];
  progress: Progress[];
  rowState: RowStateFn;
  checks: CheckState[];
  filter: Filter;
}

type Actions = Pick<
  CurrentReview,
  | "jump"
  | "jumpToComment"
  | "jumpToRow"
  | "show"
  | "commentOn"
  | "send"
  | "runWalkthrough"
  | "refetchPr"
  | "pullPr"
>;

/** Publishes this pane as the workspace's current review while it's active, when none is
 * yet, or when it already is and its data changed. Retracts it when the pane goes. */
export function usePublishReview(
  api: DockviewPanelApi,
  workspace: WorkspaceName,
  review: ReviewDto | undefined,
  { files, progress, rowState, checks, filter }: Data,
  actions: Actions,
) {
  const published = useRef<CurrentReview>(undefined);
  // Published once, so they call through to this render's actions.
  const latest = useRef(actions);
  latest.current = actions;

  useEffect(() => {
    const publish = () => {
      // A pane left outside the dock can't show anything.
      if (!inDock(workspace, api.id)) return;
      published.current = {
        id: review?.id ?? 0,
        revset: review?.revset ?? "",
        to: review?.to ?? "",
        files,
        progress,
        rowState,
        checks,
        filter,
        comments: review?.state.comments ?? [],
        notes: review?.state.notes ?? [],
        runWalkthrough: () => latest.current.runWalkthrough(),
        refetchPr: () => latest.current.refetchPr(),
        pullPr: (quiet) => latest.current.pullPr(quiet),
        jump: (f) => latest.current.jump(f),
        jumpToComment: (id) => latest.current.jumpToComment(id),
        jumpToRow: (file, row) => latest.current.jumpToRow(file, row),
        show: (path, line) => latest.current.show(path, line),
        commentOn: (file, rows, body, snippet) =>
          latest.current.commentOn(file, rows, body, snippet),
        send: (text, onSent, report) => latest.current.send(text, onSent, report),
      };
      publishReview(workspace, published.current);
      // Agents in the workspace's terminals act on this review.
      if (review) commands.focusReview(workspace, review.id);
    };
    // Active, unclaimed or held by the idle review, or already ours and changed.
    const ours = isCurrent(workspace, published.current);
    const free = !currentReview(workspace) || isIdle(currentReview(workspace));
    if (review && (api.isActive || free || ours)) publish();
    const sub = api.onDidActiveChange(({ isActive }) => {
      if (isActive && review) publish();
    });
    // Takes over when the pane that was current closes.
    const off = onReviewChange(() => {
      if (review && !currentReview(workspace)) publish();
    });
    return () => {
      sub.dispose();
      off();
    };
  }, [api, workspace, review, files, progress, rowState, checks, filter]);

  useEffect(() => () => retractReview(workspace, published.current), [workspace]);

  /** Whether this pane is the one its workspace's agents and side views act on. */
  return () => isCurrent(workspace, published.current);
}
