import type { PrDto, WorkingCopyDto } from "../bindings";
import { dockOf } from "../core/docks";
import { openReviewTab } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import {
  askDescribe,
  askRevset,
  copyItems,
  presets,
  REV_ACTIONS,
  type RevAction,
  reviewAfter,
} from "../review/RevisionsPanelMenu";
import type { ReviewTarget } from "../review/target";
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
export function reviewRevset(target: ReviewTarget, revset: string) {
  run(
    target.load(revset).then((failed) => {
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
  openRevisions(): void;
}

/** The header's menu. Its commit actions act on `@`; Review picks the PR, a preset or any
 * revset. */
export function revsetMenu({
  workspace,
  target,
  trunk,
  pr,
  wc,
  openRevisions,
}: Picker): CtxEntry[] {
  const head = target?.changes.at(-1);
  const jj = (f: () => Promise<RevAction | undefined>) => () => {
    if (!target) return;
    run(f().then((a) => a && actOn(workspace, target, a)));
  };
  const busy = !target || target.loading;
  const review = (revset: string) => () => target && reviewRevset(target, revset);
  const choices = [
    ...(pr ? [[`#${pr.number}`, pr.revset]] : []),
    ...presets(trunk).map((p) => [p, p]),
  ];
  return [
    { label: "Open Revisions", keys: "⌘J", disabled: !target, onSelect: openRevisions },
    {
      label: "Open Review Tab",
      onSelect: () => {
        const api = dockOf(workspace);
        if (api) openReviewTab(api);
      },
    },
    {
      label: "Review",
      disabled: !target,
      items: [
        ...choices.map(([label, revset]) => ({
          label,
          checked: target?.revset === revset,
          onSelect: review(revset),
        })),
        "-",
        {
          label: "Revset…",
          onSelect: () => {
            if (!target) return;
            askRevset(target.revset).then((r) => r?.trim() && reviewRevset(target, r.trim()));
          },
        },
      ],
    },
    "-",
    wc && {
      label: `@ ${wc.changeId.slice(0, Math.max(wc.unique, 5))}`,
      disabled: true,
      onSelect() {},
    },
    wc && {
      label: "Describe…",
      disabled: busy,
      onSelect: jj(() => askDescribe("@", wc.description)),
    },
    { label: "New Change on Top", disabled: busy, onSelect: jj(async () => REV_ACTIONS.n("@")) },
    { label: "Squash into Parent", disabled: busy, onSelect: jj(async () => REV_ACTIONS.s("@")) },
    head && {
      label: "Edit Review Head",
      disabled: busy || head.workingCopy,
      onSelect: jj(async () => REV_ACTIONS.e(head.changeId)),
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
