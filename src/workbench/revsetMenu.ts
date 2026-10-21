import type { PrDto, WorkingCopyDto } from "../bindings";
import { dockOf } from "../core/docks";
import { openReviewTab } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import {
  askRevset,
  copyItems,
  editAction,
  presets,
  type RevAction,
  reviewAfter,
} from "../review/RevisionsPanelMenu";
import type { ReviewTarget } from "../review/target";
import { loadReview } from "../revisions/loadReview";
import { run } from "../services/pathMenu";
import type { CtxEntry } from "../ui/contextMenu";

/** Runs a jj action, then reloads the review as the revisions panel does. */
export async function actOn(
  workspace: WorkspaceName,
  target: ReviewTarget,
  [action, , follow]: RevAction,
) {
  await unwrap(commands.jjAct(workspace, action));
  await reviewAfter(workspace, target.load, target.revset, follow);
}

/** Loads `revset` as the review, throwing its error. */
export function reviewRevset(workspace: WorkspaceName, revset: string) {
  run(
    loadReview(workspace, revset).then((failed) => {
      if (failed) throw new Error(failed);
    }),
  );
}

interface Picker {
  workspace: WorkspaceName;
  target: ReviewTarget | undefined;
  trunk: string | null;
  pr: PrDto | null | undefined;
  wc: WorkingCopyDto | undefined;
}

/** The revisions log's menu: Review picks the PR, a preset or any revset; Edit Review Head moves `@`
 * to the review's newest commit. */
export function revsetMenu({ workspace, target, trunk, pr, wc }: Picker): CtxEntry[] {
  const head = target?.changes.at(-1);
  const jj = (f: () => Promise<RevAction | undefined>) => () => {
    if (!target) return;
    run(f().then((a) => a && actOn(workspace, target, a)));
  };
  const busy = !target || target.loading;
  const review = (revset: string) => () => reviewRevset(workspace, revset);
  const current = target?.revset;
  const choices = [
    ...(pr ? [[`#${pr.number}`, pr.revset]] : []),
    ...presets(trunk).map((p) => [p, p]),
  ];
  return [
    {
      label: "Open Review Tab",
      onSelect: () => {
        const api = dockOf(workspace);
        if (api) openReviewTab(api);
      },
    },
    {
      label: "Review",
      items: [
        ...choices.map(([label, revset]) => ({
          label,
          checked: current === revset,
          keys: revset === "@" ? "⇧⌘J" : undefined,
          onSelect: review(revset),
        })),
        "-",
        {
          label: "Revset…",
          onSelect: () =>
            askRevset(current ?? "@").then((r) => r?.trim() && reviewRevset(workspace, r.trim())),
        },
      ],
    },
    "-",
    wc && {
      label: `@ ${wc.changeId.slice(0, Math.max(wc.unique, 5))}`,
      disabled: true,
      onSelect() {},
    },
    head && {
      label: "Edit Review Head",
      disabled: busy || head.workingCopy,
      onSelect: jj(async () => editAction(head.changeId)),
    },
    "-",
    wc && { label: "Copy", items: copyItems(wc.changeId, wc.commitId, wc.description) },
    "-",
    {
      label: "Undo Last jj Operation",
      disabled: busy,
      onSelect: jj(async () => [{ kind: "undo", steps: 1 }, "Undid"]),
    },
  ];
}
