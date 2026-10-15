import { useState } from "react";
import type { PrDto, Stage, ThreadDto } from "../bindings";
import { PrSection } from "../github/PrSection";
import { viewerLogin } from "../github/people";
import { usePr } from "../github/pr";
import { useDiscussion } from "../github/useDiscussion";
import { commands, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";
import { useCurrentReview } from "../review/current";
import { prThreads, quoted } from "../review/prComments";
import { openSettings } from "../settings/open";
import { useAgentsOff, useGithubOff } from "../tools/agents";
import { type CtxEntry, contextMenu, copy } from "../ui/contextMenu";
import { openUrl, run } from "../ui/pathMenu";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { SendBox } from "./SendBox";
import { byStage, STAGES, threadLabel, threadWhere, working } from "./stages";
import { TakePill } from "./ThreadBits";
import { finalText } from "./text";
import { useThreadRun } from "./useThreadRun";
import "../review/comments.css";
import "./threads.css";
import { Spinner } from "../ui/Busy";

function threadMenu(review: CurrentReview, t: ThreadDto): CtxEntry[] {
  const setStage = (s: Stage) =>
    run(unwrap(commands.setThreadStage(review.id, t.id, s)).then(review.apply));
  return [
    (t.placed || !t.path) && {
      label: t.path ? "Show Code" : "Show Comment",
      onSelect: () => review.jumpToComment(t.id),
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
  runner: ReturnType<typeof useThreadRun>;
  picked: boolean;
  onPick(): void;
  /** A quote, under the comment it's from. */
  sub?: boolean;
  /** Shows its stage as a dot. */
  dot?: boolean;
}

function ThreadRow({ review, t, runner, picked, onPick, sub, dot }: RowProps) {
  const tone = STAGES.find(([s]) => s === t.state.stage)?.[2];
  const inRun = runner.run?.threads.has(t.id);
  return (
    <button
      type="button"
      className={`thread-row${inRun ? " in-run" : ""}${sub ? " sub" : ""}`}
      aria-current={picked}
      onClick={() => {
        onPick();
        if (t.placed || !t.path) review.jumpToComment(t.id);
      }}
      onContextMenu={contextMenu(() => threadMenu(review, t))}
    >
      <span className="item-text">
        {!t.github && !t.state.quote && <span className="pill purple">Draft</span>}{" "}
        {t.github && !t.path && `${t.github.posts[0]?.author ?? "PR"}: `}
        {threadLabel(t)}
      </span>
      <span className="thread-where">
        <TakePill take={t.state.take} />
        {t.path && threadWhere(t)}
        {inRun && <Spinner />}
        {dot && <span className={`dot stage-seg ${tone || "grey"}`} />}
      </span>
    </button>
  );
}

interface ListProps {
  review: CurrentReview;
  threads: ThreadDto[];
  runner: ReturnType<typeof useThreadRun>;
}

const CONVERSATION = "conversation";

/** Comments on the PR with their quotes, then threads on code grouped by stage in work order,
 * Done folded. Picking a thread shows its comment in the review. */
function ThreadGroups({ review, threads, runner }: ListProps) {
  const [folded, setFolded] = useState<ReadonlySet<string>>(new Set(["done"]));
  const [picked, setPicked] = useState<number>();
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
      runner={runner}
      picked={picked === t.id}
      onPick={() => setPicked(t.id)}
      {...extra}
    />
  );
  const convo = prThreads(threads);
  const head = (key: string, name: string, tone: string | undefined, actions?: React.ReactNode) => {
    const shut = folded.has(key);
    return (
      <div className="stage-head">
        <button
          type="button"
          className="stage-toggle"
          aria-expanded={!shut}
          onClick={() => toggle(key)}
        >
          <span className="chevron">{shut ? "▸" : "▾"}</span>
          {tone !== undefined && <span className={`dot stage-seg ${tone || "grey"}`} />}
          <span>{name}</span>
        </button>
        <span className="spacer" />
        {actions}
      </div>
    );
  };
  return (
    <>
      {convo.length > 0 && (
        <div>
          {head(
            CONVERSATION,
            "Conversation",
            undefined,
            <span className="stage-count">{convo.filter((t) => !t.state.quote).length}</span>,
          )}
          {!folded.has(CONVERSATION) &&
            convo.map((t) => row(t, { sub: !!t.state.quote, dot: !split.has(t.id) }))}
        </div>
      )}
      {byStage(threads.filter((t) => t.path)).map(([stage, group]) => {
        const [, name, tone] = STAGES.find(([s]) => s === stage) ?? [];
        return (
          <div key={stage}>
            {head(stage, name ?? stage, tone, <span className="stage-count">{group.length}</span>)}
            {!folded.has(stage) && group.map((t) => row(t))}
          </div>
        );
      })}
    </>
  );
}

function PrBody({ pr, review }: { pr: PrDto; review: CurrentReview | undefined }) {
  const workspace = useWorkspace();
  const off = useGithubOff();
  const agentsOff = useAgentsOff();
  const discussion = useDiscussion(workspace, pr);
  const [confirm, setConfirm] = useState(0);
  const [error, setError] = useState<string>();
  const runner = useThreadRun(review, setError);
  const threads = review?.threads ?? [];
  const viewer = discussion.d?.viewer ?? (review && viewerLogin(review.comments));
  const sending = review && !off;
  const fresh = working(threads).filter((t) => t.state.stage === "new").length;
  const canTriage = !!review && !agentsOff && fresh > 0 && !runner.run;
  const sync = () => {
    review?.refetchPr();
    discussion.load();
  };
  const headerMenu = contextMenu(() => [
    { label: "Sync", disabled: discussion.loading, onSelect: sync },
    { label: "Open PR on GitHub", onSelect: () => run(unwrap(commands.openPrPage(workspace))) },
    { label: "Copy PR URL", onSelect: () => copy(pr.url) },
    { label: "Copy Branch Name", onSelect: () => copy(pr.headRef) },
    "-",
    {
      label: "Copy Final Replies",
      title: "Final replies, threads fixed as asked, and what's still open, as text",
      onSelect: () => copy(finalText(threads)),
    },
    { label: "Edit Reply Prompt…", onSelect: () => openSettings(workspace, "prompts") },
    sending && { label: "Send Ready to GitHub…", onSelect: () => setConfirm((n) => n + 1) },
  ]);

  function onKey(e: React.KeyboardEvent) {
    const t = e.target;
    if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement) return;
    if (e.key !== "t" || e.metaKey || e.ctrlKey || e.altKey || !canTriage) return;
    e.preventDefault();
    runner.start();
  }

  return (
    <div style={{ display: "contents" }} onKeyDown={onKey}>
      <div style={{ display: "contents" }} onContextMenu={headerMenu}>
        <SectionHeader title="PR">
          {!off && (
            <button type="button" className="pr-head-btn" onClick={sync}>
              Sync
            </button>
          )}
          {canTriage && (
            <button type="button" className="pr-head-btn triage" onClick={() => runner.start()}>
              Triage {fresh} <kbd>t</kbd>
            </button>
          )}
        </SectionHeader>
      </div>
      {runner.run && (
        <div className="threads-run">
          <Spinner />
          <span>
            Triaging {runner.run.threads.size}{" "}
            {runner.run.threads.size === 1 ? "thread" : "threads"}
            {runner.run.activity && ` · ${runner.run.activity}`}
          </span>
          <span className="spacer" />
          <button type="button" onClick={runner.stop}>
            Stop
          </button>
        </div>
      )}
      {error && <div className="review-error">{error}</div>}
      <div className="comments-scroll">
        <PrSection pr={pr} review={review} discussion={discussion} threads={working(threads)} />
        {review ? (
          <ThreadGroups review={review} threads={threads} runner={runner} />
        ) : (
          <Empty>No review open</Empty>
        )}
      </div>
      {sending && (
        <SendBox
          review={review}
          threads={threads}
          repo={pr.repo}
          own={!!viewer && pr.author === viewer}
          onSent={discussion.load}
          confirm={confirm}
        />
      )}
    </div>
  );
}

/** A PR workspace's PR: its state and conversation, every thread on it by stage, and the box
 * that sends what's ready. */
export function PrPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const pr = usePr(workspace);
  return (
    <div className="comments-view">
      {pr ? (
        <PrBody pr={pr} review={review} />
      ) : (
        <>
          <SectionHeader title="PR" />
          <Empty>No PR selected</Empty>
        </>
      )}
    </div>
  );
}
