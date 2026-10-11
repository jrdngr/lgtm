import type { DockviewPanelApi } from "dockview-react";
import { useEffect, useRef } from "react";
import type { CheckState, ReviewDto } from "../bindings";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import {
  type CurrentReview,
  currentReview,
  isCurrent,
  publishReview,
  retractReview,
} from "./current";
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
  "jump" | "jumpToComment" | "jumpToRow" | "commentOn" | "send" | "findRisks"
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
        hotspots: review?.state.hotspots ?? [],
        findRisks: () => latest.current.findRisks(),
        jump: (f) => latest.current.jump(f),
        jumpToComment: (id) => latest.current.jumpToComment(id),
        jumpToRow: (file, row) => latest.current.jumpToRow(file, row),
        commentOn: (file, rows, body, snippet) =>
          latest.current.commentOn(file, rows, body, snippet),
        send: (text, onSent, report) => latest.current.send(text, onSent, report),
      };
      publishReview(workspace, published.current);
      // Agents in the workspace's terminals act on this review.
      if (review) commands.focusReview(workspace, review.id);
    };
    // Active, unclaimed, or already ours and changed.
    const ours = isCurrent(workspace, published.current);
    if (review && (api.isActive || !currentReview(workspace) || ours)) publish();
    const sub = api.onDidActiveChange(({ isActive }) => {
      if (isActive && review) publish();
    });
    return () => sub.dispose();
  }, [api, workspace, review, files, progress, rowState, checks, filter]);

  useEffect(() => () => retractReview(workspace, published.current), [workspace]);

  /** Whether this pane is the one its workspace's agents and side views act on. */
  return () => isCurrent(workspace, published.current);
}
