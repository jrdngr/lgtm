import { Menu } from "../ui/Menu";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { commentText } from "./comments";
import { useCurrentReview } from "./current";
import { NoteList } from "./NoteList";
import "./comments.css";

/** The current review's notes: yours, for yourself and the agent, never sent anywhere. */
export function NotesPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const notes = (review?.comments ?? []).filter((c) => !c.thread);
  const open = notes.filter((c) => !c.done);
  const sendOpen = () =>
    review?.send(open.map((c) => commentText(review.files, c)).join("\n\n---\n\n"));

  return (
    <div className="comments-view">
      <SectionHeader title="Notes" meta={review ? `${open.length} open` : undefined}>
        <Menu
          items={[
            {
              label: "Send open notes to a terminal",
              title: "Paste every open note into a terminal's prompt",
              onSelect: sendOpen,
              disabled: open.length === 0,
            },
          ]}
        />
      </SectionHeader>
      <div className="comments-scroll">
        {review ? <NoteList review={review} notes={notes} /> : <Empty>No review open</Empty>}
      </div>
    </div>
  );
}
