import type { MarkColor, StateDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { type CommandResult, commands, unwrap } from "../ipc";
import { loadReview } from "../revisions/loadReview";
import { absolutePath, pathItems, run } from "../services/pathMenu";
import { type Sender, sendItems } from "../terminal/sendItems";
import { type CtxEntry, copy } from "../ui/contextMenu";
import { confirmAction } from "../ui/prompt";
import type { CurrentReview } from "./current";
import type { MarkSetup } from "./marks";
import { toggledMark } from "./model";
import { reviewAfter } from "./RevisionsPanelMenu";
import { dirPaths, type TreeDir } from "./tree";

const apply = (review: CurrentReview, r: Promise<CommandResult<StateDto>>) =>
  run(unwrap(r).then(review.apply));

/** Marks every changed row of `file`, or clears them when they all have the mark. The first mark, when it counts as done, also checks
 * the file, as marking a whole file in the review does. */
function markFile(review: CurrentReview, setup: MarkSetup, file: number, color: MarkColor) {
  const rows = review.files[file].rows.flatMap((r, row) =>
    r.kind === "context" ? [] : [{ file, row }],
  );
  const set = toggledMark(rows, color, review.rowState);
  const check = set !== null && set === setup.primary ? [file] : [];
  apply(review, commands.markRows(review.id, rows, set, check));
}

/** Undoes what the review's commits do to the file at `path` (and where it was renamed from),
 * once confirmed. */
async function discard(ws: WorkspaceName, review: CurrentReview, path: string, old: string | null) {
  if (!(await confirmAction(`Discard changes to ${path}?`, "Discard"))) return;
  const paths = old && old !== path ? [path, old] : [path];
  await unwrap(commands.jjAct(ws, { kind: "discard", revs: review.revset, paths }));
  await reviewAfter(ws, (r) => loadReview(ws, r), review.revset);
}

export function fileMenu(
  ws: WorkspaceName,
  review: CurrentReview,
  setup: MarkSetup,
  file: number,
  send: Sender,
): CtxEntry[] {
  const { path, oldPath, status } = review.files[file].dto;
  const checked = review.checks[file] === "checked";
  return [
    { label: "Jump to File", onSelect: () => review.jump(file) },
    {
      label: "Check File",
      keys: "x",
      checked,
      onSelect: () => apply(review, commands.setFilesChecked(review.id, [file], !checked)),
    },
    {
      label: "Mark File With",
      items: setup.shown.map((s, i) => ({
        label: setup.label(s.color),
        keys: `${i + 1}`,
        onSelect: () => markFile(review, setup, file, s.color),
      })),
    },
    {
      label: "Clear Marks in File",
      onSelect: () => apply(review, commands.clearFileMarks(review.id, file)),
    },
    "-",
    {
      label: "Discard Changes",
      onSelect: () => run(discard(ws, review, path, status === "renamed" ? oldPath : null)),
    },
    "-",
    status !== "removed" && { label: "Open File", onSelect: () => openFile(ws, path) },
    ...(status === "removed"
      ? sendItems(ws, "Send Path", path, send)
      : pathItems(ws, path, { relative: false, files: true, send })),
  ];
}

/** The review's files under `dir`. */
function filesIn(dir: TreeDir): number[] {
  return dir.children.flatMap((c) => (c.kind === "dir" ? filesIn(c) : [c.file]));
}

export function dirMenu(
  ws: WorkspaceName,
  review: CurrentReview,
  dir: TreeDir,
  closed: ReadonlySet<string>,
  setClosed: (next: ReadonlySet<string>) => void,
): CtxEntry[] {
  const files = filesIn(dir);
  const check = (on: boolean) => () =>
    apply(review, commands.setFilesChecked(review.id, files, on));
  const inside = [dir.path, ...dirPaths(dir)];
  return [
    { label: "Check All Files", onSelect: check(true) },
    { label: "Uncheck All Files", onSelect: check(false) },
    "-",
    {
      label: "Expand All",
      onSelect: () => setClosed(new Set([...closed].filter((p) => !inside.includes(p)))),
    },
    { label: "Collapse All", onSelect: () => setClosed(new Set([...closed, ...inside])) },
    "-",
    { label: "Copy Path", onSelect: () => copy(absolutePath(ws, dir.path)) },
  ];
}
