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

/** Submits the pending review on GitHub with a verdict and summary. Sits at the bottom of the
 * Comments view. */
export function SubmitReview({ workspace, comments, onSubmitted }: Props) {
  const [verdict, setVerdict] = useState<Verdict>("comment");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string>();
  const pending = comments.flatMap((c) => c.github?.posts ?? []).filter((p) => p.pending).length;
  const label = VERDICTS.find(([v]) => v === verdict)?.[1];

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
      <fieldset className="verdicts" aria-label="Verdict">
        {VERDICTS.map(([v, text]) => (
          <button
            key={v}
            type="button"
            aria-pressed={verdict === v}
            className={`verdict-${v}`}
            onClick={() => setVerdict(v)}
          >
            {text}
          </button>
        ))}
      </fieldset>
      <textarea
        className="comment-input"
        rows={2}
        value={body}
        placeholder="Summary (optional)"
        onChange={(e) => setBody(e.target.value)}
      />
      {note && <div className="submit-note">{note}</div>}
      <div className="submit-row">
        <span className="submit-count">{pending} pending</span>
        <span className="spacer" />
        <button
          type="button"
          className="primary"
          onClick={submit}
          disabled={busy}
          title="Submit your review on GitHub"
        >
          {busy ? "Submitting…" : label}
        </button>
      </div>
    </div>
  );
}
