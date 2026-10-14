import { useState } from "react";
import type { PrDto, Stage, ThreadDto } from "../bindings";
import { PrSection } from "../github/PrSection";
import { viewerLogin } from "../github/people";
import { usePr } from "../github/pr";
import { useDiscussion } from "../github/useDiscussion";
import { commands, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";
import { useCurrentReview } from "../review/current";
import { openSettings } from "../settings/open";
import { useAgentsOff, useGithubOff } from "../tools/agents";
import { type CtxEntry, contextMenu, copy } from "../ui/contextMenu";
import { openUrl, run } from "../ui/pathMenu";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { SendBox } from "./SendBox";
import { byStage, STAGES, threadLabel, threadWhere } from "./stages";
import { TakePill } from "./ThreadBits";
import { finalText, threadText } from "./text";
import { useThreadRun } from "./useThreadRun";
import "../review/comments.css";
import "./threads.css";
import { Spinner } from "../ui/Busy";

function threadMenu(review: CurrentReview, t: ThreadDto): CtxEntry[] {
  const setStage = (s: Stage) =>
    run(unwrap(commands.setThreadStage(review.id, t.id, s)).then(review.apply));
  return [
    t.placed && { label: "Show Code", onSelect: () => review.jumpToComment(t.id) },
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

interface ListProps {
  review: CurrentReview;
  threads: ThreadDto[];
  runner: ReturnType<typeof useThreadRun>;
  onError(e: string | undefined): void;
}

/** Threads grouped by stage in work order, Done folded. Picking a thread shows its comment in
 * the review. The To fix group can be fixed by an agent or sent to a terminal. */
function ThreadGroups({ review, threads, runner, onError }: ListProps) {
  const agentsOff = useAgentsOff();
  const [folded, setFolded] = useState<ReadonlySet<Stage>>(new Set(["done"]));
  const [picked, setPicked] = useState<number>();
  if (threads.length === 0) {
    return (
      <Empty>
        No threads yet. Sync, or select code and press <kbd>C</kbd>
      </Empty>
    );
  }
  const toggle = (s: Stage) =>
    setFolded((f) => {
      const next = new Set(f);
      if (!next.delete(s)) next.add(s);
      return next;
    });
  const toFix = threads.filter((t) => t.state.stage === "toFix");
  const sendToFix = () =>
    review.send(toFix.map(threadText).join("\n\n---\n\n"), undefined, (o) =>
      onError("error" in o ? o.error : undefined),
    );
  return (
    <>
      {byStage(threads).map(([stage, group]) => {
        const [, name, tone] = STAGES.find(([s]) => s === stage) ?? [];
        const shut = folded.has(stage);
        return (
          <div key={stage}>
            <div className="stage-head">
              <button
                type="button"
                className="stage-toggle"
                aria-expanded={!shut}
                onClick={() => toggle(stage)}
              >
                <span className="chevron">{shut ? "▸" : "▾"}</span>
                <span className={`dot stage-seg ${tone || "grey"}`} />
                <span>{name}</span>
              </button>
              <span className="spacer" />
              {stage === "toFix" && !agentsOff && (
                <button
                  type="button"
                  className="stage-act"
                  disabled={!!runner.run}
                  onClick={() => runner.start("fix")}
                  title="An agent changes the code in @ for these threads"
                >
                  Fix {toFix.length}
                </button>
              )}
              {stage === "toFix" && (
                <button
                  type="button"
                  className="stage-act"
                  onClick={sendToFix}
                  title="Paste these threads into a terminal's prompt"
                >
                  Send {toFix.length} to terminal
                </button>
              )}
              <span className="stage-count">{group.length}</span>
            </div>
            {!shut &&
              group.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  className="thread-row"
                  aria-current={picked === t.id}
                  onClick={() => {
                    setPicked(t.id);
                    if (t.placed) review.jumpToComment(t.id);
                  }}
                  onContextMenu={contextMenu(() => threadMenu(review, t))}
                >
                  <span className="item-text">
                    {!t.github && <span className="pill purple">Draft</span>} {threadLabel(t)}
                  </span>
                  <span className="thread-where">
                    <TakePill take={t.state.take} />
                    {threadWhere(t)}
                  </span>
                </button>
              ))}
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
  const fresh = threads.filter((t) => t.state.stage === "new").length;
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
    runner.start("triage");
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
            <button
              type="button"
              className="pr-head-btn triage"
              onClick={() => runner.start("triage")}
              title="An agent drafts a take and a reply for each new thread and sorts it into a stage"
            >
              Triage {fresh} <kbd>t</kbd>
            </button>
          )}
        </SectionHeader>
      </div>
      {runner.run && (
        <div className="threads-run">
          <Spinner />
          <span>
            {runner.run.kind === "triage" ? "Triaging" : "Fixing"}
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
        <PrSection pr={pr} review={review} discussion={discussion} threads={threads} />
        {review ? (
          <ThreadGroups review={review} threads={threads} runner={runner} onError={setError} />
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
