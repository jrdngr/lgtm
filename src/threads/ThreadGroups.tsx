import { useState } from "react";
import type { Stage, ThreadDto } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { prThreads, quoted } from "../model/prComments";
import { byStage, STAGES, threadLabel, threadWhere, VERDICTS } from "../model/stages";
import type { CurrentReview } from "../review/current";
import { setChatOpen, setCurrentThread, useLink } from "../review/linkedAgent";
import { openUrl, run } from "../services/pathMenu";
import { type CtxEntry, contextMenu, copy } from "../ui/contextMenu";
import { GroupHead } from "../ui/GroupHead";
import { Icon } from "../ui/icons";
import { Empty } from "../ui/SectionHeader";

function threadMenu(review: CurrentReview, t: ThreadDto): CtxEntry[] {
  const setStage = (s: Stage) =>
    run(unwrap(commands.setThreadStage(review.id, t.id, s)).then(review.apply));
  return [
    {
      label: t.path ? "Show Code" : "Show Comment",
      onSelect: () => review.jumpToComment(t.id),
    },
    t.messages.length > 0 && {
      label: "Show Chat",
      onSelect: () => {
        setChatOpen(t.id, true);
        review.jumpToComment(t.id);
      },
    },
    {
      label: "Move to Stage",
      items: STAGES.map(([s, name]) => ({
        label: name,
        checked: s === t.state.stage,
        onSelect: () => setStage(s),
      })),
    },
    "-",
    t.github && { label: "Open on GitHub", onSelect: () => t.github && openUrl(t.github.url) },
    t.github && { label: "Copy Link", onSelect: () => t.github && copy(t.github.url) },
  ];
}

interface RowProps {
  review: CurrentReview;
  t: ThreadDto;
  picked: boolean;
  /** The thread the linked agent hears I'm on. */
  current: boolean;
  onPick(): void;
  /** A quote, under the comment it's from. */
  sub?: boolean;
  /** Shows its stage as a dot. */
  dot?: boolean;
}

function ThreadRow({ review, t, picked, current, onPick, sub, dot }: RowProps) {
  const tone = STAGES.find(([s]) => s === t.state.stage)?.[2];
  const author = t.github && !t.state.quote ? t.github.posts[0]?.author : undefined;
  return (
    <button
      type="button"
      className={`thread-row${sub ? " sub" : ""}`}
      aria-current={picked}
      onClick={() => {
        onPick();
        review.jumpToComment(t.id);
      }}
      onContextMenu={contextMenu(() => threadMenu(review, t))}
    >
      <span
        className={`take-dot ${t.state.take ? VERDICTS[t.state.take.verdict][1] || "grey" : ""}`}
      />
      <span className="item-text">
        {!t.github && !t.state.quote && <span className="pill purple">Draft</span>}
        {author && <b className="thread-who">{author}</b>}
        {threadLabel(t)}
      </span>
      <span className="thread-where">
        {t.fixes.length > 0 && (
          <svg className="fix-ic" viewBox="0 0 16 16" aria-label="Fixed">
            <path d="M8 2.5v6M5 5.5h6M5 13h6" />
          </svg>
        )}
        {t.path && threadWhere(t)}
        {t.suggestion && <span className="row-suggested" />}
        {current && (
          <span className="row-synced">
            <Icon name="terminal" size={13} />
          </span>
        )}
        {dot && <span className={`dot stage-seg ${tone || "grey"}`} />}
      </span>
    </button>
  );
}

interface ListProps {
  review: CurrentReview;
  threads: ThreadDto[];
}

const CONVERSATION = "conversation";

/** Comments on the PR with their quotes, then threads on code grouped by stage in work order,
 * Done folded. Picking a thread shows its comment in the review and makes it the one I'm on. */
export function ThreadGroups({ review, threads }: ListProps) {
  const workspace = useWorkspace();
  const [folded, setFolded] = useState<ReadonlySet<string>>(new Set(["done"]));
  const link = useLink(workspace);
  if (threads.length === 0) {
    return (
      <Empty>
        No threads yet. Sync, or select code and press <kbd>C</kbd>
      </Empty>
    );
  }
  const toggle = (s: string) =>
    setFolded((f) => {
      const next = new Set(f);
      if (!next.delete(s)) next.add(s);
      return next;
    });
  const split = quoted(threads);
  const row = (t: ThreadDto, extra?: Partial<RowProps>) => (
    <ThreadRow
      key={t.id}
      review={review}
      t={t}
      picked={link.current === t.id}
      current={!!link.terminal && link.current === t.id}
      onPick={() => setCurrentThread(workspace, t.id)}
      {...extra}
    />
  );
  // Done comments on the PR go under Done with the threads.
  const convo = prThreads(threads).filter((t) => t.state.stage !== "done");
  return (
    <>
      {convo.length > 0 && (
        <div>
          <GroupHead
            name="Conversation"
            count={convo.filter((t) => !t.state.quote).length}
            shut={folded.has(CONVERSATION)}
            onToggle={() => toggle(CONVERSATION)}
          />
          {!folded.has(CONVERSATION) &&
            convo.map((t) => row(t, { sub: !!t.state.quote, dot: !split.has(t.id) }))}
        </div>
      )}
      {byStage(threads.filter((t) => t.path || t.state.stage === "done")).map(([stage, group]) => {
        const [, name, tone] = STAGES.find(([s]) => s === stage) ?? [];
        return (
          <div key={stage}>
            <GroupHead
              name={name ?? stage}
              tone={tone}
              count={group.length}
              shut={folded.has(stage)}
              onToggle={() => toggle(stage)}
            />
            {!folded.has(stage) && group.map((t) => row(t))}
          </div>
        );
      })}
    </>
  );
}
