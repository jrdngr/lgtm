import type { CommentDto } from "../bindings";
import { Busy } from "../ui/Busy";
import { Icon } from "../ui/icons";
import { FileDiff } from "./FileDiff";
import type { Comments } from "./useComments";
import "./proposal.css";

const files = (n: number) => `${n} ${n === 1 ? "file" : "files"}`;
const changes = (n: number) => `${n} ${n === 1 ? "change" : "changes"}`;
const base = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** The code changes a thread's reply promises, as diffs under the chat. They're written when
 * the reply goes final; written ones stay folded, and ones whose file changed since can go to
 * the agent. */
export function ProposalSection({ comment, c }: { comment: CommentDto; c: Comments }) {
  const proposal = comment.thread?.proposal ?? [];
  if (proposal.length === 0) return null;
  const id = comment.id;
  const applying = c.applying.has(id);
  const waiting = proposal.filter((p) => p.state === "proposed");
  const stale = proposal.filter((p) => p.state === "stale");
  const done = waiting.length === 0 && stale.length === 0;
  return (
    <section className="flow-sec proposal">
      <h4 className="flow-h">
        {done ? "Changes" : "Proposed changes"}{" "}
        <span className="flow-ver">
          {done ? "· applied to @" : `· ${files(waiting.length + stale.length)}`}
        </span>
      </h4>
      {applying && <Busy className="flow-thinking">Applying {changes(waiting.length)}…</Busy>}
      {stale.map((p) => (
        <div key={p.path} className="pd-warn">
          <span>{base(p.path)} changed after the proposal, so it wasn't written.</span>
          <span className="spacer" />
          <button type="button" className="flow-draft" onClick={() => c.applyByAgent(id, p)}>
            <Icon name="sparkle" size={12} />
            Ask agent to apply
          </button>
          <button type="button" onClick={() => c.discard(id, p.path)}>
            Discard
          </button>
        </div>
      ))}
      {proposal.map((p) => (
        <FileDiff
          key={p.path}
          p={p}
          folded={p.state === "applied"}
          side={
            p.state === "applied" ? (
              <span className="pill green">✓ Applied</span>
            ) : p.state === "stale" ? (
              <span className="pill amber">Changed since</span>
            ) : (
              !applying && (
                <button type="button" onClick={() => c.discard(id, p.path)}>
                  Discard
                </button>
              )
            )
          }
        />
      ))}
      {waiting.length > 0 && !applying && (
        <div className="faint">Applies to @ when the reply is final</div>
      )}
    </section>
  );
}
