import { useState } from "react";
import type { CommentDto } from "../bindings";
import { isMac, SAVE_KEY } from "../core/keys";
import type { WorkspaceName } from "../core/workspace";
import { commands, errorText } from "../ipc";
import { submitNextSend } from "../terminal/registry";
import { notesText } from "./comments";
import { currentReview } from "./current";
import { linkedTerminal } from "./linkedAgent";

interface Props {
  workspace: WorkspaceName;
  review: number;
  comment: CommentDto;
}

/** A box under the agent's last reply on a note: what I write joins the conversation and goes
 * to the agent, which answers under it again. */
export function NoteReply({ workspace, review, comment }: Props) {
  const [text, setText] = useState("");
  const [error, setError] = useState<string>();

  async function reply() {
    const body = text.trim();
    if (!body) return;
    const r = await commands.addChatMessage(review, comment.id, "user", body);
    if (r.status === "error") return setError(r.error);
    setText("");
    setError(undefined);
    const current = currentReview(workspace);
    if (!current) return;
    const asked = {
      ...comment,
      messages: [...comment.messages, { author: "user" as const, text: body }],
    };
    submitNextSend();
    current.send(
      notesText(current.files, [asked], current.revset),
      undefined,
      (o) => setError("error" in o ? errorText(o.error) : undefined),
      linkedTerminal(workspace) ?? undefined,
    );
  }

  return (
    <div className="card-foot editing note-reply">
      <textarea
        className="comment-input"
        value={text}
        rows={Math.min(12, Math.max(3, text.split("\n").length))}
        placeholder="Write a reply"
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (isMac ? e.metaKey : e.ctrlKey)) {
            e.preventDefault();
            reply();
          }
          e.stopPropagation();
        }}
      />
      {error && <div className="review-error">{error}</div>}
      <div className="card-buttons">
        <span className="spacer" />
        <button type="button" className="primary" disabled={!text.trim()} onClick={reply}>
          Reply <kbd>{SAVE_KEY}</kbd>
        </button>
      </div>
    </div>
  );
}
