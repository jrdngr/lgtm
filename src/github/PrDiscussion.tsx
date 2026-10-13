import { useState } from "react";
import { prOpen } from "../workbench/subviews";
import { PrPosts } from "./PrPosts";
import { verdicts } from "./people";
import type { Discussion } from "./useDiscussion";

const TONES = { approved: "green", changes: "red", commented: "" };
const SAYS = { approved: "approved", changes: "requested changes", commented: "commented" };

/** A PR's conversation, folded to one line of each reviewer's verdict. Open, its posts and a
 * box that posts to it. */
export function PrDiscussion({ discussion, viewer }: { discussion: Discussion; viewer?: string }) {
  const { d, error, post } = discussion;
  const open = prOpen.use();
  const [reply, setReply] = useState("");
  const [busy, setBusy] = useState(false);

  async function send() {
    setBusy(true);
    if (await post(reply)) setReply("");
    setBusy(false);
  }

  return (
    <div className="pr-convo">
      <button
        type="button"
        className="pr-convo-bar"
        aria-expanded={open}
        onClick={() => prOpen.set(!open)}
        title={open ? "Fold the conversation" : "Show the conversation"}
      >
        <span className="chevron">{open ? "▾" : "▸"}</span>
        <b>Conversation</b>
        {d && <span className="faint">{d.posts.length}</span>}
        {d &&
          verdicts(d.posts).map(([who, v]) => (
            <span key={who} className={`pill ${TONES[v]}`} title={`${who} ${SAYS[v]}`}>
              {v === "approved" && "✓ "}
              {who}
            </span>
          ))}
      </button>
      {error && <div className="review-error">{error}</div>}
      {open && !d && !error && <div className="pr-empty">Loading</div>}
      {open && d && (
        <>
          <PrPosts posts={d.posts} viewer={viewer} />
          <div className="pr-reply">
            <textarea
              className="comment-input"
              rows={2}
              value={reply}
              placeholder="Reply on the PR (posts right away)"
              onChange={(e) => setReply(e.target.value)}
            />
            <button type="button" onClick={send} disabled={busy || !reply.trim()}>
              {busy ? "Posting…" : "Post"}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
