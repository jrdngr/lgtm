import { useState } from "react";
import type { CommentDto, PrDto, ThreadDto } from "../bindings";
import { remembered } from "../core/remembered";
import { useWorkspace } from "../core/workspace";
import { PrSection } from "../github/PrSection";
import { useDiscussion } from "../github/useDiscussion";
import { commands, unwrap } from "../ipc";
import { viewerLogin } from "../model/people";
import { working } from "../model/stages";
import { triageAllText } from "../review/agentText";
import type { CurrentReview } from "../review/current";
import { sendToAgent, startLink } from "../review/linkCommands";
import { useLink } from "../review/linkedAgent";
import { NoNotes, NoteList } from "../review/NoteList";
import { useNotes } from "../review/useNotes";
import { run } from "../services/pathMenu";
import { openSettings } from "../settings/open";
import { PipeButton } from "../terminal/PipeButton";
import { useGithubOff } from "../tools/agents";
import { contextMenu, copy } from "../ui/contextMenu";
import { GroupHead } from "../ui/GroupHead";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { LinkStrip } from "./LinkStrip";
import { SendBox } from "./SendBox";
import { ThreadGroups } from "./ThreadGroups";
import { finalText } from "./text";

type Filter = "all" | "pr" | "notes";

const shownFilter = remembered<Filter>("lgtm:commentsFilter", "all", ["all", "pr", "notes"]);
const notesFolded = remembered("lgtm:notesFolded", false, [true, false]);

function FilterBar({ pr, notes }: { pr: number; notes: number }) {
  const filter = shownFilter.use();
  const option = (f: Filter, label: string, count?: number) => (
    <button
      type="button"
      className={filter === f ? "option on" : "option"}
      aria-pressed={filter === f}
      onClick={() => shownFilter.set(f)}
    >
      {label}
      {count !== undefined && <b>{count}</b>}
    </button>
  );
  return (
    <div className="comments-filter">
      {option("all", "All")}
      {option("pr", "PR", pr)}
      {option("notes", "Notes", notes)}
    </div>
  );
}

interface Props {
  pr: PrDto;
  review: CurrentReview | undefined;
}

/** A PR workspace's comments: the PR's state and conversation, every thread on it by stage,
 * your notes, and the box that sends what's ready. */
export function PrComments({ pr, review }: Props) {
  const workspace = useWorkspace();
  const off = useGithubOff();
  const discussion = useDiscussion(workspace, pr);
  const filter = shownFilter.use();
  const shut = notesFolded.use();
  const notes = useNotes(review);
  const [confirm, setConfirm] = useState(0);
  const [error, setError] = useState<string>();
  const threads = review?.threads ?? [];
  const viewer = discussion.d?.viewer ?? (review && viewerLogin(review.comments));
  const sending = review && !off;
  const link = useLink(workspace);
  const linked = !!link.terminal;
  const freshThreads = working(threads).filter((t) => t.state.stage === "new");
  const fresh = freshThreads.length;
  const canTriage = !!review && !link.working && fresh > 0;
  const onError = (message: string) => setError(message);
  /** Sends the new threads to the linked terminal, or the send target, to triage. */
  async function triage() {
    if (!review) return;
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
    "-",
    ...notes.menu(),
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

  const showPr = filter !== "notes";
  const showNotes = filter !== "pr";
  const openThreads = working(threads).filter((t) => t.state.stage !== "done").length;
  return (
    <div style={{ display: "contents" }} onKeyDown={onKey}>
      <div style={{ display: "contents" }} onContextMenu={headerMenu}>
        <SectionHeader title="Comments">
          {!off && (
            <button type="button" className="pr-head-btn" onClick={sync}>
              Sync
            </button>
          )}
          {canTriage && (
            <PipeButton className="pr-head-btn" onClick={triage} keys="t">
              Triage {fresh}
            </PipeButton>
          )}
          {review && !linked && (
            <PipeButton
              className="pr-head-btn"
              onClick={() => startLink(workspace, onError)}
              keys="l"
            >
              Link
            </PipeButton>
          )}
        </SectionHeader>
      </div>
      <LinkStrip workspace={workspace} />
      {(error ?? notes.error) && <div className="review-error">{error ?? notes.error}</div>}
      <FilterBar pr={openThreads} notes={notes.open.length} />
      <div className="comments-scroll">
        {showPr && (
          <PrSection pr={pr} review={review} discussion={discussion} threads={working(threads)} />
        )}
        {showPr &&
          (review ? (
            <ThreadGroups review={review} threads={threads} />
          ) : (
            <Empty>No review open</Empty>
          ))}
        {showNotes &&
          review &&
          (notes.notes.length === 0 ? (
            filter === "notes" && <NoNotes />
          ) : (
            <div className="notes-group">
              <GroupHead
                name="Notes"
                count={`${notes.open.length} open`}
                shut={shut}
                onToggle={() => notesFolded.set(!shut)}
                onContextMenu={contextMenu(notes.menu)}
              />
              {!shut && (
                <NoteList
                  review={review}
                  notes={notes.notes}
                  canDraft
                  showDone={notes.showDone}
                  setShowDone={notes.setShowDone}
                />
              )}
            </div>
          ))}
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
