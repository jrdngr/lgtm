import { useMemo } from "react";
import { useCurrentReview } from "./current";
import { STATUS_LABEL } from "./DiffLines";
import { fileProgress } from "./model";

/** The files in the current review, with reviewed counts. Click one to jump to it. */
export function FilesPanel() {
  const review = useCurrentReview();
  const progress = useMemo(
    () => (review ? fileProgress(review.files, review.marks) : []),
    [review],
  );
  if (!review) return <div className="placeholder">No review open</div>;
  if (review.files.length === 0) return <div className="placeholder">No changes</div>;
  return (
    <ul className="file-list">
      {review.files.map((f, i) => {
        const { reviewed, changed } = progress[i];
        const done = changed > 0 && reviewed === changed;
        return (
          <li key={f.dto.path}>
            <button type="button" onClick={() => review.jump(i)} title={f.dto.path}>
              <span className={`status status-${f.dto.status}`}>{STATUS_LABEL[f.dto.status]}</span>
              <span className="path">{f.dto.path}</span>
              <span className={`count${done ? " done" : ""}`}>
                {reviewed}/{changed}
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
