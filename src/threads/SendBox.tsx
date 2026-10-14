import { useEffect, useState } from "react";
import type { Fixed, ThreadDto, Verdict } from "../bindings";
import { commands } from "../ipc";
import type { CurrentReview } from "../review/current";
import { remembered } from "../workbench/remembered";
import { canSend, planText, sendPlan } from "./stages";

const FIXED: [Fixed, string][] = [
  ["leave", "Leave as is"],
  ["reply", "Reply “Done”"],
  ["resolve", "Resolve"],
  ["replyResolve", "Reply “Done” and resolve"],
];

const VERDICTS: [Verdict, string][] = [
  ["comment", "Comment"],
  ["approve", "Approve"],
  ["requestChanges", "Request changes"],
];

/** What happens to fixed threads, remembered per repo. */
const fixedByRepo = new Map<string, ReturnType<typeof remembered<Fixed>>>();
function fixedFor(repo: string) {
  let r = fixedByRepo.get(repo);
  if (!r) {
    r = remembered<Fixed>(
      `lgtm:fixed:${repo}`,
      "leave",
      FIXED.map(([f]) => f),
    );
    fixedByRepo.set(repo, r);
  }
  return r;
}

interface Props {
  review: CurrentReview;
  threads: ThreadDto[];
  repo: string;
  /** On the viewer's own PR there's no verdict to give. */
  own: boolean;
  onSent(): void;
  /** Changes to open the list of what goes out, as ⌘↵ in the Threads tab does. */
  confirm?: number;
}

/** Sends every ready thread to GitHub in one go, after a short list of what goes out. On
 * someone else's PR it carries a verdict and summary. */
export function SendBox({ review, threads, repo, own, onSent, confirm }: Props) {
  const fixedStore = fixedFor(repo);
  const fixed = fixedStore.use();
  const [verdict, setVerdict] = useState<Verdict>("comment");
  const [summary, setSummary] = useState("");
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string>();
  const plan = sendPlan(threads, fixed);
  const ready = threads.filter((t) => t.state.stage === "ready").length;
  const says = !own && (verdict !== "comment" || !!summary.trim());
  const any = canSend(plan) || says;
  // biome-ignore lint/correctness/useExhaustiveDependencies: only ⌘↵ opens the list, not `any` changing
  useEffect(() => {
    if (confirm && any) {
      setNote(undefined);
      setConfirming(true);
    }
  }, [confirm]);
  const label = own ? "Send to GitHub" : (VERDICTS.find(([v]) => v === verdict)?.[1] ?? "");

  async function send() {
    setBusy(true);
    const r = await commands.sendThreads(review.id, fixed, own ? "comment" : verdict, summary);
    setBusy(false);
    setConfirming(false);
    if (r.status === "error") return setNote(r.error);
    review.apply(r.data);
    setSummary("");
    setNote("Sent.");
    onSent();
  }

  return (
    <div className="submit-review send-box">
      {!own && (
        <>
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
            value={summary}
            placeholder="Summary (optional)"
            onChange={(e) => setSummary(e.target.value)}
          />
        </>
      )}
      {plan.fixed > 0 && (
        <label className="send-fixed">
          <span>Fixed threads</span>
          <select value={fixed} onChange={(e) => fixedStore.set(e.target.value as Fixed)}>
            {FIXED.map(([f, text]) => (
              <option key={f} value={f}>
                {text}
              </option>
            ))}
          </select>
        </label>
      )}
      {note && <div className="submit-note">{note}</div>}
      {confirming ? (
        <div className="send-confirm">
          <span>{planText(plan) || (says ? "Your verdict" : "Nothing")} goes out.</span>
          <span className="spacer" />
          <button type="button" className="quiet" onClick={() => setConfirming(false)}>
            Cancel
          </button>
          <button type="button" className="primary" onClick={send} disabled={busy || !any}>
            {busy ? "Sending…" : "Send"}
          </button>
        </div>
      ) : (
        <div className="submit-row">
          <span className="submit-count">{ready} ready</span>
          <span className="spacer" />
          <button
            type="button"
            className="primary"
            onClick={() => {
              setNote(undefined);
              setConfirming(true);
            }}
            disabled={!any}
            title="Send every ready thread to GitHub"
          >
            {label}
          </button>
        </div>
      )}
    </div>
  );
}
