import { useEffect, useState } from "react";
import type { DiscussionDto, PrDto } from "../bindings";
import { commands } from "../ipc";
import { useCurrentReview } from "../review/current";
import { useWorkspace } from "../workspaces/context";
import { PrPosts } from "./PrPosts";

/** A PR's conversation and review summaries, with a box that posts to it. */
export function PrDiscussion({ pr }: { pr: PrDto }) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const [d, setD] = useState<DiscussionDto>();
  const [error, setError] = useState<string>();
  const [reply, setReply] = useState("");
  const [busy, setBusy] = useState(false);

  async function load() {
    const r = await commands.prDiscussion(workspace);
    if (r.status === "error") return setError(r.error);
    setError(undefined);
    setD(r.data);
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: reload when the workspace's PR does
  useEffect(() => {
    setD(undefined);
    if (pr) load();
  }, [pr]);

  async function post() {
    setBusy(true);
    const r = await commands.replyPr(workspace, reply);
    setBusy(false);
    if (r.status === "error") return setError(r.error);
    setReply("");
    load();
  }

  return (
    <div className="pr-panel">
      <div className="pr-head">
        <div className="pr-title">{pr.title}</div>
        <div className="pr-meta">
          {pr.author} · {pr.headRef} → {pr.baseRef}
        </div>
        <div className="pr-meta" title="The PR's page">
          {pr.url}
        </div>
        <div className="pr-actions">
          <button type="button" onClick={load}>
            Reload
          </button>
          <button
            type="button"
            disabled={!review}
            onClick={() => review?.refetchPr()}
            title="Fetch the PR's head again and pull its comments"
          >
            Refetch
          </button>
        </div>
      </div>
      {error && <div className="review-error">{error}</div>}
      {!d && !error && <div className="placeholder">Loading</div>}
      {d && <PrPosts posts={d.posts} />}
      {d && (
        <div className="pr-reply">
          <textarea
            className="comment-input"
            rows={3}
            value={reply}
            placeholder="Reply on the PR (posts right away)"
            onChange={(e) => setReply(e.target.value)}
          />
          <button type="button" onClick={post} disabled={busy || !reply.trim()}>
            {busy ? "Posting…" : "Post"}
          </button>
        </div>
      )}
    </div>
  );
}
