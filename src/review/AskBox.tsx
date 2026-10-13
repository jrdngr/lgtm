import type { RefObject } from "react";
import { isMac } from "../workbench/keys";
import type { Comments } from "./useComments";

export const NOT_AT_WORKING_COPY =
  "The agent edits the working copy (@), which this review doesn't include. Review a revset that includes @ to make changes.";

export const SAVE_KEY = isMac ? "⌘↵" : "Ctrl+↵";

interface Props {
  id: number;
  /** The comment's text, which a change with nothing typed carries out. */
  body: string;
  c: Comments;
  inputRef: RefObject<HTMLTextAreaElement | null>;
}

/** The box under a comment that asks the agent about it, or tells it what to change. */
export function AskBox({ id, body, c, inputRef }: Props) {
  const running = c.running.get(id);
  const asked = !!c.asks.get(id)?.trim();

  function onKey(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (e.key === "Enter" && mod) {
      if (c.canChange) c.ask(id, true);
    } else if (e.key === "Enter" && !e.shiftKey) c.ask(id, false);
    else if (e.key === "Escape") c.closeAsk(id);
    else return;
    e.preventDefault();
  }

  return (
    <div className="ask">
      <textarea
        ref={inputRef}
        className="ask-input"
        rows={1}
        value={c.asks.get(id) ?? ""}
        placeholder={`Ask the agent (↵), or tell it what to change (${SAVE_KEY})`}
        onChange={(e) => c.setAsk(id, e.target.value)}
        onKeyDown={onKey}
      />
      {running ? (
        <button type="button" onClick={() => c.stop(id)}>
          Stop
        </button>
      ) : (
        <>
          <button type="button" onClick={() => c.ask(id, false)} disabled={!asked}>
            Ask
          </button>
          <button
            type="button"
            onClick={() => c.ask(id, true)}
            disabled={!c.canChange || (!asked && !body.trim())}
            title={
              c.canChange
                ? "Have the agent edit the code. With nothing typed, it does what the comment says."
                : NOT_AT_WORKING_COPY
            }
          >
            Change
          </button>
        </>
      )}
    </div>
  );
}
