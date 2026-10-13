import { type RefObject, useEffect, useState } from "react";
import type { StateDto, ThreadDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import { Post } from "../github/Post";
import { authorName } from "../github/people";
import { type CommandResult, commands } from "../ipc";
import type { CurrentReview } from "../review/current";
import { Workshop } from "../review/Workshop";
import { isMac } from "../workbench/keys";
import { ReplyHead, TakePill } from "./ThreadBits";
import { unanswered } from "./text";
import type { ThreadChat } from "./useThreadChat";

interface Props {
  thread: ThreadDto;
  review: CurrentReview;
  viewer?: string;
  chat: ThreadChat;
  replyRef: RefObject<HTMLTextAreaElement | null>;
  askRef: RefObject<HTMLTextAreaElement | null>;
  /** Gives keys back to the list. */
  onLeave(): void;
  onError(message: string): void;
}

/** The thread the cursor is on, opened in place. Left: what's unanswered and the code. Right:
 * the take, the reply, and the agent chat. */
export function FocusedThread(p: Props) {
  const { thread: t, review, viewer, chat } = p;
  const [text, setText] = useState(t.body);
  const [ask, setAsk] = useState("");
  const [earlier, setEarlier] = useState(false);
  const running = chat.running.get(t.id);
  const posts = t.github?.posts ?? [];
  const [before, after] = unanswered(posts, viewer);
  const draft = !t.github;
  const take = t.state.take;
  useEffect(() => setText(t.body), [t.body]);

  async function apply(r: Promise<CommandResult<StateDto>>) {
    const got = await r;
    if (got.status === "error") return p.onError(got.error);
    review.apply(got.data);
  }

  async function save() {
    if (text !== t.body) await apply(commands.editComment(review.id, t.id, text));
  }

  async function send(change: boolean) {
    const message = ask.trim();
    if (!message && !change) return;
    setAsk("");
    await save();
    if (!(await chat.ask(t.id, message, text, change))) setAsk(message);
  }

  const mod = (e: React.KeyboardEvent) => (isMac ? e.metaKey : e.ctrlKey);

  function onReplyKey(e: React.KeyboardEvent) {
    if (e.key === "Escape" || (e.key === "Enter" && mod(e))) {
      e.preventDefault();
      e.stopPropagation();
      save();
      p.onLeave();
    }
  }

  function onAskKey(e: React.KeyboardEvent) {
    if (e.key === "Enter" && mod(e)) send(true);
    else if (e.key === "Enter" && !e.shiftKey) send(false);
    else if (e.key === "Escape") p.onLeave();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  return (
    <div className="focused-thread">
      <div className="focused-left">
        {before.length > 0 && (
          <button type="button" className="quiet earlier" onClick={() => setEarlier(!earlier)}>
            {earlier ? "▾" : "▸"} {before.length} earlier
          </button>
        )}
        {(earlier ? posts : after).map((post, i) => (
          <Post
            // biome-ignore lint/suspicious/noArrayIndexKey: posts are append-only
            key={i}
            author={post.author}
            name={authorName(post.author, viewer)}
            at={post.createdAt}
            pills={post.pending && <span className="pill amber">Pending</span>}
            body={post.body}
          />
        ))}
        {t.snippet && (
          <div className="focused-code">
            {t.github?.outdated && <span className="pill">Outdated</span>}
            <pre>{t.snippet}</pre>
          </div>
        )}
      </div>
      <div className="focused-right">
        {take && (
          <div className="focused-take">
            <TakePill take={take} />
            <Markdown>{take.why}</Markdown>
            {take.question && (
              <div className="take-question">
                <Markdown>{take.question}</Markdown>
              </div>
            )}
          </div>
        )}
        {!draft && <ReplyHead state={t.state} />}
        <textarea
          ref={p.replyRef}
          className="comment-input"
          rows={Math.min(10, Math.max(3, text.split("\n").length))}
          value={text}
          placeholder={draft ? "Write a comment for GitHub" : "Write a reply (e)"}
          onChange={(e) => setText(e.target.value)}
          onBlur={save}
          onKeyDown={onReplyKey}
        />
        <Workshop messages={t.messages} running={running} onUse={setText} />
        <div className="ask">
          <textarea
            ref={p.askRef}
            className="ask-input"
            rows={1}
            value={ask}
            placeholder="Tell the agent what you think (↵), or what to change in the code (⌘↵)"
            onChange={(e) => setAsk(e.target.value)}
            onKeyDown={onAskKey}
          />
          {running && (
            <button type="button" onClick={() => chat.stop(t.id)}>
              Stop
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
