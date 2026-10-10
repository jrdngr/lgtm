import { useState } from "react";
import type { Finding } from "../bindings";
import { useCurrentReview } from "../review/current";
import { rowsText } from "../review/sendText";
import { useWorkspace } from "../workspaces/context";
import { BADGES, type Exchange, findingText, locationText, place } from "./findings";
import { Inline, Markdown } from "./Markdown";

const STANCES = { concede: "conceded", defend: "defended", partlyConcede: "partly conceded" };

function ExchangeView({ e }: { e: Exchange }) {
  return (
    <li>
      <span className="finding-who">
        {e.kind === "challenge" ? `${e.by} challenged` : `${e.by} ${STANCES[e.stance]}`}
      </span>
      <Markdown>{e.kind === "challenge" ? e.argument : e.evidence}</Markdown>
    </li>
  );
}

interface Props {
  finding: Finding;
  /** Who it's from: a persona, or "synthesis". */
  source: string;
  exchanges: Exchange[];
  rulings: { outcome: string; reason: string }[];
  open: boolean;
  onToggle(): void;
  /** Picked to send to the agent with others. */
  checked: boolean;
  onCheck(): void;
  /** Pastes text into a terminal's prompt. */
  onSend(text: string): void;
}

/** A finding: a header with its badge, title, and places in the code, which opens to show the
 * evidence, the fix, and the cross-review behind it. */
export function FindingCard(props: Props) {
  const {
    finding: f,
    source,
    exchanges,
    rulings,
    open,
    onToggle,
    checked,
    onCheck,
    onSend,
  } = props;
  const review = useCurrentReview(useWorkspace());
  const [commented, setCommented] = useState(false);
  const placed = f.locations.map((loc) => (review ? place(review.files, loc) : undefined));
  const first = placed.find((p) => p);
  const notHere = review ? "Not in the open review" : "Open a review to see the code";
  const raisers = f.raisedBy?.length ? f.raisedBy : undefined;

  async function comment() {
    if (!review || !first) return;
    const snippet = rowsText(review.files[first.file], first.changed);
    const ok = await review.commentOn(first.file, first.changed, findingText(f, source), snippet);
    setCommented(ok);
  }

  return (
    <div className={`finding finding-${f.severity}`} data-finding={f.id}>
      <div className="finding-top">
        <input
          type="checkbox"
          className="finding-check"
          checked={checked}
          onChange={onCheck}
          title="Pick it to send to the agent with others"
        />
        <button type="button" className="finding-head" onClick={onToggle} aria-expanded={open}>
          <span className="finding-badge">{BADGES[f.severity]}</span>
          <span className="finding-title">
            <Inline>{f.title}</Inline>
          </span>
          <span className="finding-id">{f.id?.includes("#") ? f.id : `#${f.id}`}</span>
        </button>
      </div>
      <div className="finding-meta">
        {raisers ? (
          raisers.map((r) => (
            <span
              key={r.persona}
              className="finding-chip"
              title={`${r.persona} rated it ${r.severity}`}
            >
              {r.persona}
              {!r.inLens && " (off-lens)"}
            </span>
          ))
        ) : (
          <span className="finding-chip">
            {source}
            {f.offLens && " (off-lens)"}
          </span>
        )}
        {f.locations.map((loc, i) => (
          <button
            type="button"
            // biome-ignore lint/suspicious/noArrayIndexKey: a finding's locations never reorder
            key={i}
            disabled={!placed[i]}
            title={placed[i] ? "Show in the review" : notHere}
            onClick={() => {
              const at = placed[i];
              if (at) review?.jumpToRow(at.file, at.row);
            }}
          >
            {locationText(loc)}
          </button>
        ))}
        <span className="spacer" />
        <button
          type="button"
          disabled={!review}
          title="Paste this finding into a terminal's prompt"
          onClick={() => onSend(findingText(f, source))}
        >
          Send to terminal
        </button>
        <button
          type="button"
          disabled={!first || commented}
          title={first ? "Add this finding as a comment on its code" : notHere}
          onClick={comment}
        >
          {commented ? "Commented" : "Comment"}
        </button>
      </div>
      {open && (
        <div className="finding-body">
          {f.facts.length > 0 && (
            <ul>
              {f.facts.map((x) => (
                <li key={x}>
                  <Markdown>{x}</Markdown>
                </li>
              ))}
            </ul>
          )}
          <Markdown>{f.explanation}</Markdown>
          {f.contested && <Markdown>{`**Contested:** ${f.contested}`}</Markdown>}
          {f.fix && <Markdown>{`**Fix:** ${f.fix}`}</Markdown>}
          {(exchanges.length > 0 || rulings.length > 0) && (
            <div className="finding-cross">
              <div className="finding-who">Cross-review</div>
              <ul>
                {exchanges.map((e, i) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: exchanges never reorder
                  <ExchangeView key={i} e={e} />
                ))}
              </ul>
              {rulings.map((r, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: rulings never reorder
                <Markdown key={i}>{`**Ruling: ${r.outcome}.** ${r.reason}`}</Markdown>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
