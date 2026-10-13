import { useEffect, useState } from "react";
import type { StateDto, ThreadDto } from "../bindings";
import { Markdown } from "../bots/Markdown";
import { GhThread } from "../github/GhThread";
import { type CommandResult, commands } from "../ipc";
import type { CurrentReview } from "../review/current";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { isMac } from "../workbench/keys";
import { STAGES } from "./stages";
import { ReplyHead } from "./ThreadBits";

interface Props {
  thread: ThreadDto;
  review: CurrentReview;
  viewer?: string;
}

/** A thread opened in a list: its posts, the take, and the reply, which saves on blur or with
 * ⌘↵ and can be made final. */
export function ThreadCard({ thread: t, review, viewer }: Props) {
  const [text, setText] = useState(t.body);
  const [error, setError] = useState<string>();
  const draft = !t.github;
  // A save elsewhere, or by an agent, shows here.
  useEffect(() => setText(t.body), [t.body]);

  async function apply(r: Promise<CommandResult<StateDto>>) {
    const got = await r;
    if (got.status === "error") return setError(got.error);
    setError(undefined);
    review.apply(got.data);
  }

  async function save() {
    if (text !== t.body) await apply(commands.editComment(review.id, t.id, text));
  }

  async function final() {
    await save();
    await apply(commands.setThreadStage(review.id, t.id, "ready"));
  }

  function onKey(e: React.KeyboardEvent) {
    if (e.key === "Enter" && (isMac ? e.metaKey : e.ctrlKey)) {
      e.preventDefault();
      save();
    }
  }

  return (
    <div className="thread-card">
      <div className="thread-card-bar">
        <span className="spacer" />
        {t.placed && (
          <IconButton
            icon="eye"
            label="Show in the review"
            onClick={() => review.jumpToComment(t.id)}
          />
        )}
        <Menu
          items={STAGES.filter(([s]) => s !== t.state.stage).map(([s, label]) => ({
            label: `Move to ${label}`,
            onSelect: () => apply(commands.setThreadStage(review.id, t.id, s)),
          }))}
        />
      </div>
      {t.github && <GhThread link={t.github} viewer={viewer} />}
      {t.state.take?.question && (
        <div className="take-question thread-question">
          <Markdown>{t.state.take.question}</Markdown>
        </div>
      )}
      {!draft && <ReplyHead state={t.state} />}
      <div className="thread-reply">
        <textarea
          className="comment-input"
          rows={Math.min(10, Math.max(2, text.split("\n").length))}
          value={text}
          placeholder={draft ? "Write a comment for GitHub" : "Write a reply"}
          onChange={(e) => setText(e.target.value)}
          onBlur={save}
          onKeyDown={onKey}
        />
        {!draft && text.trim() && t.state.stage !== "ready" && (
          <button type="button" className="primary" onClick={final}>
            Final
          </button>
        )}
      </div>
      {error && <div className="review-error">{error}</div>}
    </div>
  );
}
