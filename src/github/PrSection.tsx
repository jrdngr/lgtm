import { useState } from "react";
import type { PrDto, ThreadDto } from "../bindings";
import { commands } from "../ipc";
import type { CurrentReview } from "../review/current";
import { StageBar } from "../threads/StageBar";
import { IconButton } from "../ui/icons";
import { ago } from "../ui/time";
import { useWorkspace } from "../workspaces/context";
import { Avatar } from "./Post";
import { viewerLogin } from "./people";
import type { Discussion } from "./useDiscussion";
import "./github.css";

const STATES: Record<string, [string, string]> = {
  OPEN: ["green", "Open"],
  CLOSED: ["red", "Closed"],
  MERGED: ["purple", "Merged"],
};

function synced(at: number | undefined) {
  if (at === undefined) return "";
  const a = ago(at);
  return a === "now" ? " · synced just now" : ` · synced ${a} ago`;
}

interface Props {
  pr: PrDto;
  review: CurrentReview | undefined;
  discussion: Discussion;
  threads: ThreadDto[];
}

/** A PR workspace's PR atop its threads: state, title, how far along its threads are, branches,
 * and a Sync button. */
export function PrSection({ pr, review, discussion, threads }: Props) {
  const workspace = useWorkspace();
  const [error, setError] = useState<string>();
  // Hovering Sync re-renders, so its tooltip's age is current.
  const [, hovered] = useState(0);
  const state = discussion.head && STATES[discussion.head.state];
  const viewer = discussion.d?.viewer ?? (review && viewerLogin(review.comments));

  /** Fetches the PR, which pulls its threads into the review, and reads its conversation. */
  function sync() {
    review?.refetchPr();
    discussion.load();
  }

  async function openPage() {
    const r = await commands.openPrPage(workspace);
    setError(r.status === "error" ? r.error : undefined);
  }

  return (
    <div className="pr-head">
      <div className="pr-row">
        {state && <span className={`pill ${state[0]}`}>{state[1]}</span>}
        <span className="pr-number">#{pr.number}</span>
        {pr.author === viewer && <span className="pill blue">Yours</span>}
        <span className="spacer" />
        <IconButton icon="external" label="Open on GitHub" onClick={openPage} />
        <IconButton
          icon="sync"
          label={`Sync with GitHub: fetch the PR and pull its comments${synced(discussion.syncedAt)}`}
          className={discussion.loading ? "spinning" : undefined}
          disabled={discussion.loading}
          onMouseEnter={() => hovered((n) => n + 1)}
          onClick={sync}
        />
      </div>
      <div className="pr-title">{pr.title}</div>
      <StageBar threads={threads} />
      <div className="pr-row pr-meta">
        <Avatar name={pr.author} small />
        <span>{pr.author === viewer ? "You" : pr.author}</span>
        <span className="faint">·</span>
        <span className="pr-branches" title={`${pr.headRef} → ${pr.baseRef}`}>
          {pr.headRef} → {pr.baseRef}
        </span>
      </div>
      {(error ?? discussion.error) && (
        <div className="review-error">{error ?? discussion.error}</div>
      )}
    </div>
  );
}
