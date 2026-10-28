import type { CommentDto } from "../bindings";
import { STAGES } from "../model/stages";
import { openUrl } from "../services/pathMenu";
import { type CtxEntry, copy } from "../ui/contextMenu";
import type { MenuItem } from "../ui/Menu";
import { confirmAction } from "../ui/prompt";
import type { Comments } from "./useComments";

interface Card {
  comment: CommentDto;
  c: Comments;
}

function stageItems({ comment, c }: Card) {
  const at = comment.thread?.stage;
  return STAGES.map(([s, label]) => ({
    label,
    checked: s === at,
    onSelect: () => c.setStage(comment.id, s),
  }));
}

/** Deletes a quote, or a draft not sent to GitHub, after asking. */
function deleteItem({ comment, c }: Card): MenuItem {
  const what = comment.thread?.quote ? "Quote" : "Draft";
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
  return [!card.comment.github && deleteItem(card)];
}

/** The right-click menu of a comment card: a GitHub thread or draft. */
export function cardItems(card: Card): CtxEntry[] {
  const { comment, c } = card;
  const gh = comment.github;
  const send = { label: "Send to Terminal", onSelect: () => c.send(comment) };
  return [
    {
      label: !gh ? "Edit" : comment.body ? "Edit Reply" : "Reply",
      onSelect: () => c.edit(comment),
    },
    { label: "Move to Stage", items: stageItems(card) },
    send,
    "-",
    gh && { label: "Open on GitHub", onSelect: () => openUrl(gh.url) },
    gh && { label: "Copy Link", onSelect: () => copy(gh.url) },
    "-",
    deleteItem(card),
  ];
}
