import type { EditorView } from "@codemirror/view";
import { useEffect, useMemo, useState } from "react";
import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { useCurrentReview } from "../review/current";
import { diffBlocks } from "./diffBlocks";
import { setDiff } from "./diffGutter";

/** Shows the current review's changes to `path` in the editor's gutter, in whichever view the
 * review is in. */
export function useDiffGutter(
  view: EditorView | undefined,
  workspace: WorkspaceName,
  path: string,
) {
  const review = useCurrentReview(workspace);
  const index = review?.files.findIndex((f) => f.dto.path === path) ?? -1;
  const file = index >= 0 ? review?.files[index] : undefined;
  const blocks = useMemo(() => (file ? diffBlocks(file.rows) : []), [file]);
  /** The review's new side of the file, as lines. */
  const [newSide, setNewSide] = useState<string[]>();
  const id = review?.id;
  const head = review?.head;
  const lineCount = file?.dto.lineCount ?? 0;
  /** Stands in for `blocks` as a dependency, since the review's files are rebuilt often. */
  const key = useMemo(() => JSON.stringify(blocks), [blocks]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `head` and `key` say the new side changed
  useEffect(() => {
    setNewSide(undefined);
    if (id === undefined || key === "[]") return;
    let live = true;
    unwrap(commands.contextLines(id, index, 1, Math.max(lineCount, 1))).then(
      (lines) => live && setNewSide(lines.map((l) => l.text)),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [id, head, index, lineCount, key]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `key` stands in for `blocks`
  useEffect(() => {
    view?.dispatch({ effects: setDiff.of(newSide && { blocks, newSide }) });
  }, [view, key, newSide]);
}
