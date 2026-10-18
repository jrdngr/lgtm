import { dockOf } from "../core/docks";
import { openReviewTab } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import {
  askDescribe,
  presets,
  REV_ACTIONS,
  type RevAction,
  reviewAfter,
} from "../review/RevisionsPanelMenu";
import type { ReviewTarget } from "../review/target";
import { type CtxEntry, copy } from "../ui/contextMenu";
import { run } from "../ui/pathMenu";

/** Runs a jj action, then reloads the review as the revisions panel does. */
async function act(workspace: WorkspaceName, target: ReviewTarget, [action, , next]: RevAction) {
  await unwrap(commands.jjAct(workspace, action));
  const failed = await reviewAfter(target.load, target.revset, next);
  if (failed) throw new Error(failed);
}

interface Picker {
  workspace: WorkspaceName;
  target: ReviewTarget | undefined;
  trunk: string | null;
  openRevisions(): void;
}

/** The header's revset field menu. Its commit actions act on the review's newest change, where
 * the revisions panel's cursor starts. */
export function revsetMenu({ workspace, target, trunk, openRevisions }: Picker): CtxEntry[] {
  const head = target?.changes.at(-1);
  const jj = (f: () => Promise<RevAction | undefined>) => () => {
    if (!target) return;
    run(f().then((a) => a && act(workspace, target, a)));
  };
  const onHead = (key: string) => jj(async () => head && REV_ACTIONS[key](head.changeId));
  const busy = !target || target.loading;
  const review = (revset: string) => () => {
    if (!target) return;
    run(
      target.load(revset).then((failed) => {
        if (failed) throw new Error(failed);
      }),
    );
  };
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
      items: presets(trunk).map((p) => ({
        label: p,
        checked: target?.revset === p,
        onSelect: review(p),
      })),
    },
    "-",
    ...(head
      ? [
          { label: "Copy Change ID", onSelect: () => copy(head.changeId) },
          { label: "Copy Commit ID", onSelect: () => copy(head.commitId) },
          { label: "Copy Description", onSelect: () => copy(head.description) },
          "-" as const,
          {
            label: "Describe…",
            disabled: busy,
            onSelect: jj(() => askDescribe(head.changeId, head.description)),
          },
          { label: "New Change on Top", disabled: busy, onSelect: onHead("n") },
          { label: "Squash into Parent", disabled: busy, onSelect: onHead("s") },
        ]
      : []),
    "-",
    {
      label: "Undo Last jj Operation",
      disabled: busy,
      onSelect: jj(async () => [{ kind: "undo", steps: 1 }, "Undid"]),
    },
  ];
}
