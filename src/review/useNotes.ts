import { useState } from "react";
import { remembered } from "../core/remembered";
import type { WorkspaceName } from "../core/workspace";
import type { CtxEntry } from "../ui/contextMenu";
import { commentText } from "./comments";
import type { CurrentReview } from "./current";
import { linkedTerminal } from "./linkedAgent";
import { isSent, markSent, useSentNotes } from "./sentNotes";

const doneShown = remembered("lgtm:showDoneNotes", false, [true, false]);

/** The review's notes, yours for yourself and the agent and never sent to GitHub, with their
 * menu: send the open ones not yet sent to the agent's terminal, and show or hide done ones. */
export function useNotes(review: CurrentReview | undefined, workspace: WorkspaceName) {
  const showDone = doneShown.use();
  const [error, setError] = useState<string>();
  const notes = (review?.comments ?? []).filter((c) => !c.thread);
  const open = notes.filter((c) => !c.done);
  useSentNotes();
  const unsent = open.filter((c) => !isSent(c));
  const sendOpen = () => {
    if (!review || !unsent.length) return;
    const batch = unsent;
    review.send(
      batch.map((c) => commentText(review.files, c)).join("\n\n---\n\n"),
      () => markSent(batch),
      (o) => setError("error" in o ? o.error : undefined),
      linkedTerminal(workspace) ?? undefined,
    );
  };
  const menu = (): CtxEntry[] => [
    {
      label: "Send Comments to Agent",
      onSelect: sendOpen,
      disabled: unsent.length === 0,
    },
    { label: "Show Done Comments", checked: showDone, onSelect: () => doneShown.set(!showDone) },
  ];
  return { notes, open, unsent, sendOpen, showDone, setShowDone: doneShown.set, error, menu };
}
