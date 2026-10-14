import { useState } from "react";
import { usePr } from "../github/pr";
import { contextMenu } from "../ui/contextMenu";
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
  const canDraft = !!usePr(workspace);
  const [showDone, setShowDone] = useState(false);
  const notes = (review?.comments ?? []).filter((c) => !c.thread);
  const open = notes.filter((c) => !c.done);
  const [error, setError] = useState<string>();
  const sendOpen = () =>
    review?.send(
      open.map((c) => commentText(review.files, c)).join("\n\n---\n\n"),
      undefined,
      (o) => setError("error" in o ? o.error : undefined),
    );
  const send = {
    label: "Send Open Notes to a Terminal",
    title: "Paste every open note into a terminal's prompt",
    onSelect: sendOpen,
    disabled: open.length === 0,
  };

  return (
    <div className="comments-view">
      <div
        style={{ display: "contents" }}
        onContextMenu={contextMenu(() => [
          send,
          { label: "Show Done Notes", checked: showDone, onSelect: () => setShowDone(!showDone) },
        ])}
      >
        <SectionHeader title="Notes" meta={review ? `${open.length} open` : undefined}>
          <Menu items={[send]} />
        </SectionHeader>
      </div>
      {error && <div className="review-error">{error}</div>}
      <div className="comments-scroll">
        {review ? (
          <NoteList
            review={review}
            notes={notes}
            canDraft={canDraft}
            showDone={showDone}
            setShowDone={setShowDone}
          />
        ) : (
          <Empty>No review open</Empty>
        )}
      </div>
    </div>
  );
}
