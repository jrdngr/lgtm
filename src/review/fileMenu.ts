import type { CheckState, MarkColor } from "../bindings";
import { revealInFiles } from "../core/reveal";
import type { WorkspaceName } from "../core/workspace";
import { absolutePath, reveal } from "../services/pathMenu";
import type { CtxEntry } from "../ui/contextMenu";
import { copy } from "../ui/contextMenu";
import type { MarkSetup } from "./marks";
import type { FlatFile } from "./model";
import type { Progress } from "./state";

/** The colors in use as menu items, each on its number key. */
function markItems(setup: MarkSetup, onMark: (color: MarkColor) => void): CtxEntry[] {
  return setup.shown.map((s, i) => ({
    label: setup.label(s.color),
    keys: String(i + 1),
    onSelect: () => onMark(s.color),
  }));
}

interface Deps {
  workspace: WorkspaceName;
  files: FlatFile[];
  checks: CheckState[];
  progress: Progress[];
  collapsedFiles: ReadonlySet<string>;
  setup: MarkSetup;
  toggleFile(file: number): void;
  setChecked(file: number, checked: boolean): void;
  markFile(file: number, color: MarkColor): void;
  clearFile(file: number): void;
  openFile(path: string): void;
}

/** The right-click menu of a review file's header. */
export function fileMenu(d: Deps, file: number): CtxEntry[] {
  const { path, status } = d.files[file].dto;
  const checked = d.checks[file] === "checked";
  const marked = Object.values(d.progress[file]?.marks ?? {}).some((n) => n > 0);
  const removed = status === "removed";
  return [
    {
      label: checked ? "Uncheck File" : "Check File",
      keys: "x",
      onSelect: () => d.setChecked(file, !checked),
    },
    {
      label: d.collapsedFiles.has(path) ? "Expand File" : "Collapse File",
      keys: "za",
      onSelect: () => d.toggleFile(file),
    },
    "-",
    { label: "Mark File With", items: markItems(d.setup, (c) => d.markFile(file, c)) },
    {
      label: "Clear Marks in File",
      disabled: !marked && !checked,
      onSelect: () => d.clearFile(file),
    },
    "-",
    { label: "Open File", keys: "o", disabled: removed, onSelect: () => d.openFile(path) },
    { label: "Copy Path", onSelect: () => copy(absolutePath(d.workspace, path)) },
    {
      label: "Reveal in Files",
      disabled: removed,
      onSelect: () => revealInFiles(d.workspace, path),
    },
    { label: "Reveal in Finder", disabled: removed, onSelect: () => reveal(d.workspace, path) },
  ];
}
