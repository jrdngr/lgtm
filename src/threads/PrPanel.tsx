import { useState } from "react";
import type { PrDto, Stage, ThreadDto } from "../bindings";
import { PrSection } from "../github/PrSection";
import { viewerLogin } from "../github/people";
import { usePr } from "../github/pr";
import { useDiscussion } from "../github/useDiscussion";
import { commands, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";
import { useCurrentReview } from "../review/current";
import { useGithubOff } from "../tools/agents";
import { type CtxEntry, contextMenu, copy } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { openUrl, run } from "../ui/pathMenu";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { showThread, threadsOpen } from "./focus";
import { SendBox } from "./SendBox";
import { byStage, STAGES, threadLabel, threadWhere } from "./stages";
import { TakePill } from "./ThreadBits";
import { ThreadCard } from "./ThreadCard";
import "../review/comments.css";
import "./threads.css";

function threadMenu(review: CurrentReview, t: ThreadDto, open: () => void): CtxEntry[] {
  const setStage = (s: Stage) =>
    run(unwrap(commands.setThreadStage(review.id, t.id, s)).then(review.apply));
  return [
    { label: "Open", onSelect: open },
    t.placed && { label: "Show Code", keys: "⌘-click", onSelect: () => review.jumpToComment(t.id) },
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
  viewer?: string;
}

/** Threads grouped by stage in work order, Done folded. Click a thread to open it in place. */
function ThreadGroups({ review, threads, viewer }: ListProps) {
  const workspace = useWorkspace();
  const [folded, setFolded] = useState<ReadonlySet<Stage>>(new Set(["done"]));
  const [open, setOpen] = useState<number>();
  if (threads.length === 0) {
    return (
      <Empty>
        No threads yet. Sync, or select code and press <kbd>C</kbd>
      </Empty>
    );
  }
  /** Opens the thread in the Threads tab when that's open, else here. `keep` leaves an open
   * one open. */
  const show = (t: ThreadDto, keep = false) => {
    if (threadsOpen(workspace)) showThread(workspace, t.id);
    else setOpen(keep || open !== t.id ? t.id : undefined);
  };
  const toggle = (s: Stage) =>
    setFolded((f) => {
      const next = new Set(f);
      if (!next.delete(s)) next.add(s);
      return next;
    });
  return (
    <>
      {byStage(threads).map(([stage, group]) => {
        const [, name, tone] = STAGES.find(([s]) => s === stage) ?? [];
        const shut = folded.has(stage);
        return (
          <div key={stage}>
            <button
              type="button"
              className="stage-head"
              aria-expanded={!shut}
              onClick={() => toggle(stage)}
            >
              <span className="chevron">{shut ? "▸" : "▾"}</span>
              <span className={`dot stage-seg ${tone || "grey"}`} />
              <span>{name}</span>
              <span className="spacer" />
              <span>{group.length}</span>
            </button>
            {!shut &&
              group.map((t) => (
                <div key={t.id}>
                  <button
                    type="button"
                    className="thread-row"
                    aria-expanded={open === t.id}
                    onClick={(e) => {
                      if (e.metaKey && t.placed) review.jumpToComment(t.id);
                      else show(t);
                    }}
                    onContextMenu={contextMenu(() => threadMenu(review, t, () => show(t, true)))}
                    title="Open it in the Threads tab when that's open, else here. ⌘-click shows its code in the review."
                  >
                    <span className="item-text">
                      {!t.github && <span className="pill purple">Draft</span>} {threadLabel(t)}
                    </span>
                    <span className="thread-where">
                      <TakePill take={t.state.take} />
                      {threadWhere(t)}
                    </span>
                  </button>
                  {open === t.id && <ThreadCard thread={t} review={review} viewer={viewer} />}
                </div>
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
  const discussion = useDiscussion(workspace, pr);
  const [confirm, setConfirm] = useState(0);
  const threads = review?.threads ?? [];
  const viewer = discussion.d?.viewer ?? (review && viewerLogin(review.comments));
  const sending = review && !off;
  const headerMenu = contextMenu(() => [
    {
      label: "Sync",
      disabled: discussion.loading,
      onSelect: () => {
        review?.refetchPr();
        discussion.load();
      },
    },
    { label: "Open PR on GitHub", onSelect: () => run(unwrap(commands.openPrPage(workspace))) },
    { label: "Copy PR URL", onSelect: () => copy(pr.url) },
    { label: "Copy Branch Name", onSelect: () => copy(pr.headRef) },
    "-",
    { label: "Open Threads", keys: "⇧⌘T", onSelect: () => showThread(workspace) },
    sending && { label: "Send Ready to GitHub…", onSelect: () => setConfirm((n) => n + 1) },
  ]);
  return (
    <>
      <div style={{ display: "contents" }} onContextMenu={headerMenu}>
        <Header />
      </div>
      <div className="comments-scroll">
        <PrSection pr={pr} review={review} discussion={discussion} threads={threads} />
        {review ? (
          <ThreadGroups review={review} threads={threads} viewer={viewer} />
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
    </>
  );
}

function Header({ threads = true }: { threads?: boolean }) {
  const workspace = useWorkspace();
  return (
    <SectionHeader title="PR">
      {threads && (
        <IconButton
          icon="comments"
          label="Open the Threads tab (⌘⇧T)"
          onClick={() => showThread(workspace)}
        />
      )}
    </SectionHeader>
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
          <Header threads={false} />
          <Empty>This workspace isn't a PR's</Empty>
        </>
      )}
    </div>
  );
}
