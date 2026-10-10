import type { DockviewPanelApi } from "dockview-react";
import { useEffect, useRef } from "react";
import type { CheckState, ReviewDto } from "../bindings";
import type { WorkspaceName } from "../workspaces/context";
import { type CurrentReview, publishReview, retractReview } from "./current";
import type { FlatFile } from "./model";
import type { Filter, Progress } from "./state";

interface Data {
  files: FlatFile[];
  progress: Progress[];
  checks: CheckState[];
  filter: Filter;
}

type Actions = Pick<CurrentReview, "jump" | "jumpToComment" | "send">;

/** Publishes this pane as the workspace's current review while it's active, or when none
 * is yet, and retracts it when the pane goes. */
export function usePublishReview(
  api: DockviewPanelApi,
  workspace: WorkspaceName,
  review: ReviewDto | undefined,
  { files, progress, checks, filter }: Data,
  actions: Actions,
) {
  const published = useRef<CurrentReview>(undefined);
  // Published once, so they call through to this render's actions.
  const latest = useRef(actions);
  latest.current = actions;

  useEffect(() => {
    const publish = () => {
      published.current = {
        files,
        progress,
        checks,
        filter,
        comments: review?.state.comments ?? [],
        jump: (f) => latest.current.jump(f),
        jumpToComment: (id) => latest.current.jumpToComment(id),
        send: (text) => latest.current.send(text),
      };
      publishReview(workspace, published.current);
    };
    if (review && (api.isActive || !published.current)) publish();
    const sub = api.onDidActiveChange(({ isActive }) => {
      if (isActive && review) publish();
    });
    return () => sub.dispose();
  }, [api, workspace, review, files, progress, checks, filter]);

  useEffect(() => () => retractReview(workspace, published.current), [workspace]);
}
