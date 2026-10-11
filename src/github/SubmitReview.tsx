import { useState } from "react";
import type { CommentDto, Verdict } from "../bindings";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import "./github.css";

const VERDICTS: [Verdict, string][] = [
  ["comment", "Comment"],
  ["approve", "Approve"],
  ["requestChanges", "Request changes"],
];

interface Props {
  workspace: WorkspaceName;
  comments: CommentDto[];
  /** Pulls the PR's threads again, so pending posts show as submitted. */
  onSubmitted(): void;
}

/** Submits the pending review on GitHub with a verdict and summary. */
export function SubmitReview({ workspace, comments, onSubmitted }: Props) {
  const [verdict, setVerdict] = useState<Verdict>("comment");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string>();
  const pending = comments.flatMap((c) => c.github?.posts ?? []).filter((p) => p.pending).length;

  async function submit() {
    setBusy(true);
    const r = await commands.submitReview(workspace, verdict, body);
    setBusy(false);
    if (r.status === "error") return setNote(r.error);
    setBody("");
    setNote("Submitted.");
    onSubmitted();
  }

  return (
    <div className="submit-review">
      <div className="submit-row">
        <span>
          {pending} pending {pending === 1 ? "comment" : "comments"}
        </span>
        <span className="spacer" />
        <select
          value={verdict}
          onChange={(e) => setVerdict(e.target.value as Verdict)}
          aria-label="Verdict"
        >
          {VERDICTS.map(([v, label]) => (
            <option key={v} value={v}>
              {label}
            </option>
          ))}
        </select>
      </div>
      <textarea
        className="comment-input"
        rows={2}
        value={body}
        placeholder="Summary (optional)"
        onChange={(e) => setBody(e.target.value)}
      />
      <div className="submit-row">
        {note && <span className="submit-note">{note}</span>}
        <span className="spacer" />
        <button type="button" className="primary" onClick={submit} disabled={busy}>
          {busy ? "Submitting…" : "Submit review"}
        </button>
      </div>
    </div>
  );
}
