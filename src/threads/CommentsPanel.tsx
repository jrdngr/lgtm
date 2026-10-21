import { useWorkspace } from "../core/workspace";
import { usePr } from "../github/pr";
import type { CurrentReview } from "../review/current";
import { useCurrentReview } from "../review/current";
import { NoNotes, NoteList } from "../review/NoteList";
import { SendNotesButton } from "../review/SendNotesButton";
import { useNotes } from "../review/useNotes";
import { contextMenu } from "../ui/contextMenu";
import { GroupHead } from "../ui/GroupHead";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { PrComments } from "./PrComments";
import "../review/comments.css";
import "./threads.css";

/** A workspace without a PR has only your notes. */
function NotesOnly({ review }: { review: CurrentReview | undefined }) {
  const workspace = useWorkspace();
  const notes = useNotes(review, workspace);
  const menu = contextMenu(notes.menu);
  return (
    <>
      <div style={{ display: "contents" }} onContextMenu={menu}>
        <SectionHeader title="Comments">
          <SendNotesButton
            className="pr-head-btn"
            count={notes.unsent.length}
            onSend={notes.sendOpen}
          />
        </SectionHeader>
      </div>
      {notes.error && <div className="review-error">{notes.error}</div>}
      <div className="comments-scroll notes-only">
        {!review ? (
          <Empty>No review open</Empty>
        ) : notes.notes.length === 0 ? (
          <NoNotes />
        ) : (
          <>
            <GroupHead count={`${notes.open.length} open`} onContextMenu={menu} />
            <NoteList
              review={review}
              notes={notes.notes}
              canDraft={false}
              showDone={notes.showDone}
              setShowDone={notes.setShowDone}
            />
          </>
        )}
      </div>
    </>
  );
}

/** The workspace's comments: in a PR workspace, the PR's threads and your notes; otherwise
 * your notes. */
export function CommentsPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const pr = usePr(workspace);
  return (
    <div className="comments-view">
      {pr ? <PrComments pr={pr} review={review} /> : <NotesOnly review={review} />}
    </div>
  );
}
