import { useState } from "react";
import { Markdown } from "../bots/Markdown";
import type { Comments } from "./useComments";
import { wordDiff } from "./wordDiff";

interface Props {
  id: number;
  body: string;
  suggestion: string;
  c: Comments;
}

/** The reply a Review suggests, under the current one, to apply or dismiss. */
export function SuggestedReply({ id, body, suggestion, c }: Props) {
  const [changes, setChanges] = useState(false);
  return (
    <section className="flow-sec">
      <h4 className="flow-h">
        Suggested
        <span className="spacer" />
        <button type="button" className="flow-link" onClick={() => setChanges(!changes)}>
          {changes ? "Hide changes" : "Show changes"}
        </button>
      </h4>
      <div className="flow-draft-text">
        {changes ? (
          <p className="flow-changes">
            {wordDiff(body, suggestion).map((p, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: parts never reorder
              <span key={i} className={p.kind === "same" ? undefined : `flow-${p.kind}`}>
                {p.text}
              </span>
            ))}
          </p>
        ) : (
          <Markdown>{suggestion}</Markdown>
        )}
      </div>
      <div className="flow-buttons">
        <span className="spacer" />
        <button type="button" onClick={() => c.settleSuggestion(id, false)}>
          Dismiss
        </button>
        <button type="button" className="flow-draft" onClick={() => c.settleSuggestion(id, true)}>
          Apply
        </button>
      </div>
    </section>
  );
}
