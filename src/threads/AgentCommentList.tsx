import type { AgentCommentDto } from "../bindings";
import { agentCommentLine, showResolved } from "../review/agentComments";
import type { CurrentReview } from "../review/current";
import { IconButton } from "../ui/icons";

/** Shows or hides resolved comments for the agent, here and in the review. */
export function ResolvedToggle() {
  const on = showResolved.use();
  return (
    <IconButton
      icon="check"
      label="Show resolved comments"
      aria-pressed={on}
      onClick={() => showResolved.set(!on)}
    />
  );
}

/** Open ones first, then the newest. */
const byOpenThenNewest = (a: AgentCommentDto, b: AgentCommentDto) =>
  Number(a.done) - Number(b.done) || b.number - a.number;

/** The review's comments for the agent, C4 and on. A click goes to it in the review. */
export function AgentCommentList({ review }: { review: CurrentReview }) {
  const resolved = showResolved.use();
  const shown = review.agentComments.filter((c) => resolved || !c.done).sort(byOpenThenNewest);
  return shown.map((c) => {
    const line = agentCommentLine(review.files, c);
    const name = c.path.split("/").pop();
    return (
      <button
        type="button"
        key={c.number}
        className="thread-row agent-comment-row"
        onClick={() => review.jumpToAgentComment(c.number)}
      >
        <span className={`dot${c.done ? "" : " open"}`} />
        <span className="item-text">
          {c.done && <span className="pill green">✓ Resolved</span>}
          {c.body.split("\n")[0]}
        </span>
        <span className="thread-where">
          {line === undefined ? name : `${name}:${line}`}
          <span className="thread-num">C{c.number}</span>
        </span>
      </button>
    );
  });
}
