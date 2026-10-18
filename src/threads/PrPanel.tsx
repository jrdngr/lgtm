import { useState } from "react";
import type { CommentDto, PrDto, Stage, ThreadDto } from "../bindings";
import { PrSection } from "../github/PrSection";
import { viewerLogin } from "../github/people";
import { usePr } from "../github/pr";
import { useDiscussion } from "../github/useDiscussion";
import { commands, unwrap } from "../ipc";
import { triageAllText } from "../review/agentText";
import type { CurrentReview } from "../review/current";
import { useCurrentReview } from "../review/current";
import { sendToAgent, startLink } from "../review/linkCommands";
import { setChatOpen, setCurrentThread, useLink } from "../review/linkedAgent";
import { prThreads, quoted } from "../review/prComments";
import { openSettings } from "../settings/open";
import { useAgentsOff, useGithubOff } from "../tools/agents";
import { AgentButton } from "../ui/AgentButton";
import { type CtxEntry, contextMenu, copy } from "../ui/contextMenu";
import { Icon } from "../ui/icons";
import { PipeButton } from "../ui/PipeButton";
import { openUrl, run } from "../ui/pathMenu";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { LinkStrip } from "./LinkStrip";
import { SendBox } from "./SendBox";
import { byStage, STAGES, threadLabel, threadWhere, working } from "./stages";
import { finalText } from "./text";
import { useThreadRun } from "./useThreadRun";
import "../review/comments.css";
import "./threads.css";
import { Spinner } from "../ui/Busy";

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
  runner: ReturnType<typeof useThreadRun>;
  picked: boolean;
  /** The thread the linked agent hears I'm on. */
  current: boolean;
  onPick(): void;
  /** A quote, under the comment it's from. */
  sub?: boolean;
  /** Shows its stage as a dot. */
  dot?: boolean;
}

function ThreadRow({ review, t, runner, picked, current, onPick, sub, dot }: RowProps) {
  const tone = STAGES.find(([s]) => s === t.state.stage)?.[2];
  const inRun = runner.run?.threads.has(t.id);
  return (
    <button
      type="button"
      className={`thread-row${inRun ? " in-run" : ""}${sub ? " sub" : ""}`}
      aria-current={picked}
      onClick={() => {
        onPick();
        review.jumpToComment(t.id);
      }}
      onContextMenu={contextMenu(() => threadMenu(review, t))}
    >
      <span className="item-text">
        {!t.github && !t.state.quote && <span className="pill purple">Draft</span>}{" "}
        {t.github && !t.path && `${t.github.posts[0]?.author ?? "PR"}: `}
        {threadLabel(t)}
      </span>
      <span className="thread-where">
        {t.path && threadWhere(t)}
        {t.suggestion && <span className="row-suggested" />}
        {current && (
          <span className="row-synced">
            <Icon name="terminal" size={13} />
          </span>
        )}
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
 * Done folded. Picking a thread shows its comment in the review and makes it the one I'm on. */
function ThreadGroups({ review, threads, runner }: ListProps) {
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
      runner={runner}
      picked={link.current === t.id}
      current={!!link.terminal && link.current === t.id}
      onPick={() => setCurrentThread(workspace, t.id)}
      {...extra}
    />
  );
  // Done comments on the PR go under Done with the threads.
  const convo = prThreads(threads).filter((t) => t.state.stage !== "done");
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
      {byStage(threads.filter((t) => t.path || t.state.stage === "done")).map(([stage, group]) => {
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
  const link = useLink(workspace);
  const linked = !!link.terminal;
  const freshThreads = working(threads).filter((t) => t.state.stage === "new");
  const fresh = freshThreads.length;
  const canTriage = !!review && (linked ? !link.working : !agentsOff) && fresh > 0 && !runner.run;
  const onError = (message: string) => setError(message);
  /** Triages the new threads: in the linked terminal when there is one, else headless. */
  async function triage() {
    if (!review) return;
    if (!linked) return runner.start();
    const rules = await commands.promptText("reply");
    if (rules.status === "error") return setError(rules.error);
    const pairs = freshThreads.flatMap((t) => {
      const c = review.comments.find((c) => c.id === t.id);
      return c ? [[t, c] as [ThreadDto, CommentDto]] : [];
    });
    sendToAgent(workspace, review.send, triageAllText(review.files, pairs, rules.data), null);
  }
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
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === "t" && canTriage) triage();
    else if (e.key === "l" && review && !linked) startLink(workspace, onError);
    else return;
    e.preventDefault();
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
          {canTriage &&
            (linked ? (
              <PipeButton className="pr-head-btn" onClick={triage} keys="t">
                Triage {fresh}
              </PipeButton>
            ) : (
              <AgentButton className="pr-head-btn triage" onClick={triage} keys="t">
                Triage {fresh}
              </AgentButton>
            ))}
          {review && !linked && (
            <PipeButton
              className="pr-head-btn"
              onClick={() => startLink(workspace, onError)}
              keys="l"
            >
              Link Agent
            </PipeButton>
          )}
        </SectionHeader>
      </div>
      <LinkStrip workspace={workspace} />
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
