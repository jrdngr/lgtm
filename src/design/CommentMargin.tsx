import type { DocComment } from "../bindings";
import { MarkdownView } from "../editor/MarkdownView";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import { act, type Busy } from "./store";

const STATE: Record<DocComment["state"], [string, string]> = {
  queued: ["Queued", "amber"],
  applied: ["Applied", "green"],
  answered: ["Answered", "blue"],
};

interface Props {
  ws: WorkspaceName;
  slug: string;
  comments: DocComment[];
  streams: Record<string, string>;
  busy?: Busy;
  onJump(line: number): void;
}

/** The doc's comments beside it: queued changes, what was done for them, and answers. */
export function CommentMargin({ ws, slug, comments, streams, busy, onJump }: Props) {
  if (!comments.length)
    return (
      <aside className="design-margin">
        <div className="faint">Select text to comment on it or ask about it.</div>
      </aside>
    );
  return (
    <aside className="design-margin">
      {comments.map((c) => {
        const answering = busy?.kind === "ask" && busy.about === `c${c.id}`;
        const [state, tone] = STATE[c.state];
        return (
          <div key={c.id} className={`design-comment ${c.state}`}>
            <div className="design-comment-head">
              <span className={`pill ${c.kind === "ask" ? "purple" : ""}`}>
                {c.kind === "ask" ? "Ask" : "Change"}
              </span>
              <span className={`pill ${tone}`}>{answering ? "Answering" : state}</span>
              <span className="spacer" />
              <button
                type="button"
                className="quiet"
                aria-label="Delete comment"
                onClick={() => act(ws, commands.editDesignComment(ws, slug, c.id, null))}
              >
                ×
              </button>
            </div>
            <button type="button" className="design-quote" onClick={() => onJump(c.start)}>
              {c.quote}
            </button>
            <div className="design-comment-body">{c.body}</div>
            {answering && (
              <div className="design-reply">
                <MarkdownView text={streams[`${slug}:c${c.id}`] || "…"} />
              </div>
            )}
            {c.reply && !answering && (
              <div className="design-reply">
                <MarkdownView text={c.reply} />
              </div>
            )}
          </div>
        );
      })}
    </aside>
  );
}
