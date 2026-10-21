import type { LogCommit } from "../bindings";
import { showAuthor, showDate } from "./logView";
import { when } from "./when";

/** Characters of a change id shown, its unique prefix included. */
const ID_LENGTH = 8;

/** A commit's two lines: change id, description and flags; then bookmarks, author, age, and
 * the PR pill. */
export function CommitLines({ c, pill }: { c: LogCommit; pill?: React.ReactNode }) {
  const flags = [c.divergent && "divergent"].filter((f) => f !== false);
  const author = showAuthor.use();
  const date = showDate.use();
  const meta = [author && c.author, date && when(c.time ?? 0)].filter(Boolean).join(" · ");
  return (
    <span className="lr-lines">
      <span className="lr-l1">
        <span className="lr-id">
          <span className="lr-unique">{c.changeId.slice(0, c.unique)}</span>
          {c.changeId.slice(c.unique, Math.max(c.unique, ID_LENGTH))}
        </span>
        <span className="lr-desc">
          {c.description.split("\n")[0] || <i className="lr-none">no description</i>}
        </span>
        {flags.map((f) => (
          <span key={f} className="pill red">
            {f}
          </span>
        ))}
      </span>
      <span className="lr-l2">
        <span className="lr-books">
          {c.bookmarks.map((b) => (
            <span key={b}>{b}</span>
          ))}
        </span>
        {meta && <span className="lr-meta">{meta}</span>}
        {pill}
      </span>
    </span>
  );
}
