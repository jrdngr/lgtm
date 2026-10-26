import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { run } from "../services/pathMenu";
import { useCurrentReview } from "./current";
import "./FileHeader.css";

/** The review's checkbox for `path`, as its file header has it, when the current review
 * changes the file. */
export function ReviewedCheck({ workspace, path }: { workspace: WorkspaceName; path: string }) {
  const review = useCurrentReview(workspace);
  const file = review?.files.findIndex((f) => f.dto.path === path) ?? -1;
  if (!review || file < 0) return null;
  const checked = review.checks[file] === "checked";
  const p = review.progress[file];
  const allDone = !checked && p.changed > 0 && p.done === p.changed;
  return (
    <input
      type="checkbox"
      className={`reviewed-check${allDone ? " ring" : ""}`}
      checked={checked}
      onChange={() =>
        run(unwrap(commands.setFilesChecked(review.id, [file], !checked)).then(review.apply))
      }
    />
  );
}
