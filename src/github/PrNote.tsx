import type { PrReview } from "./usePrReview";

function moved(p: PrReview) {
  const h = p.head;
  if (!h?.moved) return undefined;
  if (h.rewritten) return "was force-pushed on GitHub";
  if (!h.ahead) return "has new commits on GitHub";
  return `has ${h.ahead} new ${h.ahead === 1 ? "commit" : "commits"} on GitHub`;
}

/** Under the review bar in a PR workspace: what's running, and whether the PR moved on
 * GitHub, with a button to fetch it. */
export function PrNote({ p }: { p: PrReview }) {
  if (!p.pr) return null;
  const what = moved(p);
  const closed = p.head && p.head.state !== "OPEN" ? p.head.state.toLowerCase() : undefined;
  if (!what && !closed && !p.busy) return null;
  return (
    <div className={`review-note${what ? " warn" : ""}`}>
      {p.busy ? (
        `${p.busy}…`
      ) : (
        <>
          PR #{p.pr.number} {what ?? `is ${closed}`}
          {what && (
            <button type="button" onClick={p.refetch}>
              Refetch
            </button>
          )}
        </>
      )}
    </div>
  );
}
