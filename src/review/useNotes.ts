import { useState } from "react";
import { remembered } from "../core/remembered";
import type { CtxEntry } from "../ui/contextMenu";
import { commentText } from "./comments";
import type { CurrentReview } from "./current";

const doneShown = remembered("lgtm:showDoneNotes", false, [true, false]);

/** The review's notes, yours for yourself and the agent and never sent anywhere, with their
 * menu: send the open ones to a terminal, and show or hide done ones. */
export function useNotes(review: CurrentReview | undefined) {
  const showDone = doneShown.use();
  const [error, setError] = useState<string>();
  const notes = (review?.comments ?? []).filter((c) => !c.thread);
  const open = notes.filter((c) => !c.done);
  const sendOpen = () =>
    review?.send(
      open.map((c) => commentText(review.files, c)).join("\n\n---\n\n"),
      undefined,
      (o) => setError("error" in o ? o.error : undefined),
    );
  const menu = (): CtxEntry[] => [
    {
      label: "Send Open Notes to a Terminal",
      onSelect: sendOpen,
      disabled: open.length === 0,
    },
    { label: "Show Done Notes", checked: showDone, onSelect: () => doneShown.set(!showDone) },
  ];
  return { notes, open, showDone, setShowDone: doneShown.set, error, menu };
}
