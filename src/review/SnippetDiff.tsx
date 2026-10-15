import { useState } from "react";
import { snippetLines } from "./snippet";

/** How many lines show before the rest is expanded, like GitHub's threads. */
const TAIL = 4;

/** A comment's code as it was, laid out like a GitHub thread's diff: old and new line
 * numbers, then the code, showing its last lines until expanded. */
export function SnippetDiff({ snippet }: { snippet: string }) {
  const [all, setAll] = useState(false);
  const lines = snippetLines(snippet).filter((l) => l.kind !== "hunk" || all);
  const code = lines.filter((l) => l.kind !== "hunk").length;
  const shown = all || code <= TAIL ? lines : lines.slice(-TAIL);
  return (
    <div className="snippet-diff">
      {shown.length < lines.length && (
        <button type="button" className="snip-more" onClick={() => setAll(true)}>
          ⋯ {lines.length - shown.length}
        </button>
      )}
      {shown.map((l, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: lines have no identity of their own
        <div key={i} className={`snip-row ${l.kind}`}>
          <span className="snip-num">{l.old}</span>
          <span className="snip-num">{l.new}</span>
          <span className="snip-code">
            {l.kind === "hunk"
              ? l.text
              : `${{ context: " ", added: "+", removed: "-" }[l.kind]}${l.text}`}
          </span>
        </div>
      ))}
    </div>
  );
}
