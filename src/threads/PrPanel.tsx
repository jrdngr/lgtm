import { useState } from "react";
import type { PrDto, Stage, ThreadDto } from "../bindings";
import { PrSection } from "../github/PrSection";
import { viewerLogin } from "../github/people";
import { usePr } from "../github/pr";
import { useDiscussion } from "../github/useDiscussion";
import type { CurrentReview } from "../review/current";
import { useCurrentReview } from "../review/current";
import { useGithubOff } from "../tools/agents";
import { IconButton } from "../ui/icons";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { showThread, threadsOpen } from "./focus";
import { SendBox } from "./SendBox";
import { byStage, STAGES, threadLabel, threadWhere } from "./stages";
import { TakePill } from "./ThreadBits";
import { ThreadCard } from "./ThreadCard";
import "../review/comments.css";
import "./threads.css";

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
                      else if (threadsOpen(workspace)) showThread(workspace, t.id);
                      else setOpen(open === t.id ? undefined : t.id);
                    }}
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
  const threads = review?.threads ?? [];
  const viewer = discussion.d?.viewer ?? (review && viewerLogin(review.comments));
  return (
    <>
      <div className="comments-scroll">
        <PrSection pr={pr} review={review} discussion={discussion} threads={threads} />
        {review ? (
          <ThreadGroups review={review} threads={threads} viewer={viewer} />
        ) : (
          <Empty>No review open</Empty>
        )}
      </div>
      {review && !off && (
        <SendBox
          review={review}
          threads={threads}
          repo={pr.repo}
          own={!!viewer && pr.author === viewer}
          onSent={discussion.load}
        />
      )}
    </>
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
      <SectionHeader title="PR">
        {pr && (
          <IconButton
            icon="comments"
            label="Open the Threads tab (⌘⇧T)"
            onClick={() => showThread(workspace)}
          />
        )}
      </SectionHeader>
      {pr ? <PrBody pr={pr} review={review} /> : <Empty>This workspace isn't a PR's</Empty>}
    </div>
  );
}
