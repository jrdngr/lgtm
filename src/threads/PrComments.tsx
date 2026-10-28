import { useState } from "react";
import type { CommentDto, PrDto, ThreadDto } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { PrSection } from "../github/PrSection";
import { useDiscussion } from "../github/useDiscussion";
import { commands, unwrap } from "../ipc";
import { viewerLogin } from "../model/people";
import { untriaged, working } from "../model/stages";
import { triageAllText } from "../review/agentText";
import type { CurrentReview } from "../review/current";
import { sendToBackgroundAgent, startLink } from "../review/linkCommands";
import { useLink } from "../review/linkedAgent";
import { run } from "../services/pathMenu";
import { openSettings } from "../settings/open";
import { PipeButton } from "../terminal/PipeButton";
import { useGithubOff } from "../tools/agents";
import { contextMenu, copy } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { LinkStrip } from "./LinkStrip";
import { SendBox } from "./SendBox";
import { ThreadGroups } from "./ThreadGroups";
import { finalText } from "./text";

interface Props {
  pr: PrDto;
  review: CurrentReview | undefined;
}

/** A PR workspace's comments: the PR's state and conversation, every thread on it by stage,
 * and the box that sends what's ready. */
export function PrComments({ pr, review }: Props) {
  const workspace = useWorkspace();
  const off = useGithubOff();
  const discussion = useDiscussion(workspace, pr);
  const [confirm, setConfirm] = useState(0);
  const [error, setError] = useState<string>();
  const threads = review?.threads ?? [];
  const viewer = discussion.d?.viewer ?? (review && viewerLogin(review.comments));
  const sending = review && !off;
  const link = useLink(workspace);
  const linked = !!link.terminal;
  const freshThreads = untriaged(threads);
  const fresh = freshThreads.length;
  const canTriage = !!review && !link.working && fresh > 0;
  const onError = (message: string) => setError(message);
  /** Sends the new threads to the background terminal to triage. */
  async function triage() {
    if (!review) return;
    const rules = await commands.promptText("reply");
    if (rules.status === "error") return setError(rules.error);
    const pairs = freshThreads.flatMap((t) => {
      const c = review.comments.find((c) => c.id === t.id);
      return c ? [[t, c] as [ThreadDto, CommentDto]] : [];
    });
    sendToBackgroundAgent(workspace, triageAllText(review.files, pairs, rules.data), null, onError);
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
        <SectionHeader title="Comments">
          {!off && <IconButton icon="sync" label="Sync" onClick={sync} />}
        </SectionHeader>
        {canTriage && (
          <div className="comments-actions">
            <PipeButton background className="pr-head-btn" onClick={triage} keys="t">
              Triage {fresh}
            </PipeButton>
          </div>
        )}
      </div>
      <LinkStrip workspace={workspace} />
      {error && <div className="review-error">{error}</div>}
      <div className="comments-scroll">
        <PrSection pr={pr} review={review} discussion={discussion} threads={working(threads)} />
        {review ? (
          <ThreadGroups review={review} threads={threads} />
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
