import type { CommentDto } from "../bindings";
import { showThread } from "../threads/focus";
import { STAGES } from "../threads/stages";
import { type CtxEntry, copy } from "../ui/contextMenu";
import type { MenuItem } from "../ui/Menu";
import { openUrl } from "../ui/pathMenu";
import { confirmAction } from "../ui/prompt";
import type { WorkspaceName } from "../workspaces/context";
import type { Comments } from "./useComments";

interface Card {
  comment: CommentDto;
  c: Comments;
  workspace: WorkspaceName;
  agentsOff: boolean;
}

function stageItems({ comment, c }: Card) {
  const at = comment.thread?.stage;
  return STAGES.map(([s, label]) => ({
    label,
    checked: s === at,
    onSelect: () => c.setStage(comment.id, s),
  }));
}

/** Notes become drafts for the PR, in a PR workspace. */
function draftItem({ comment, c }: Card): MenuItem | false {
  return (
    !comment.thread &&
    c.canDraft && {
      label: "Draft PR Comment",
      title: "Start a draft for GitHub on the same lines, with this note's text",
      onSelect: () => c.draftFromNote(comment.id),
    }
  );
}

/** Deletes a note, or a draft not sent to GitHub, after asking. */
function deleteItem({ comment, c }: Card): MenuItem {
  const what = comment.thread ? "Draft" : "Note";
  return {
    label: `Delete ${what}…`,
    danger: true,
    disabled: !!comment.github,
    onSelect: async () => {
      if (await confirmAction(`Delete this ${what.toLowerCase()}?`, "Delete")) {
        c.remove(comment.id);
      }
    },
  };
}

/** The ⋯ menu of a comment card. */
export function moreItems(card: Card): (MenuItem | false)[] {
  const { comment } = card;
  return [
    ...(comment.thread ? stageItems(card) : [])
      .filter((s) => !s.checked)
      .map((s) => ({ label: `Move to ${s.label}`, onSelect: s.onSelect })),
    draftItem(card),
    !comment.github && deleteItem(card),
  ];
}

/** The right-click menu of a comment card: a note, or a GitHub thread or draft. */
export function cardItems(card: Card): CtxEntry[] {
  const { comment, c, workspace } = card;
  const gh = comment.github;
  const ask = !card.agentsOff && {
    label: "Ask the Agent",
    keys: "a",
    onSelect: () => c.openAsk(comment.id),
  };
  const send = { label: "Send to Terminal", onSelect: () => c.send(comment) };
  if (!comment.thread) {
    return [
      { label: "Edit", onSelect: () => c.edit(comment) },
      {
        label: "Done",
        checked: comment.done,
        onSelect: () => c.setDone(comment.id, !comment.done),
      },
      ask,
      send,
      draftItem(card),
      "-",
      { label: "Copy Text", disabled: !comment.body, onSelect: () => copy(comment.body) },
      "-",
      deleteItem(card),
    ];
  }
  return [
    {
      label: !gh ? "Edit" : comment.body ? "Edit Reply" : "Reply",
      onSelect: () => c.edit(comment),
    },
    { label: "Move to Stage", items: stageItems(card) },
    ask,
    send,
    "-",
    { label: "Open in Threads", onSelect: () => showThread(workspace, comment.id) },
    gh && { label: "Open on GitHub", onSelect: () => openUrl(gh.url) },
    gh && { label: "Copy Link", onSelect: () => copy(gh.url) },
    "-",
    deleteItem(card),
  ];
}
