import { useState } from "react";
import type { Finding } from "../bindings";
import { openFile } from "../editor/open";
import { useCurrentReview } from "../review/current";
import { rowsText } from "../review/sendText";
import { IconButton } from "../ui/icons";
import { useWorkspace } from "../workspaces/context";
import { type Exchange, findingText, locationText, place } from "./findings";
import { Inline, Markdown } from "./Markdown";
import { SevTag } from "./Severity";

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
  /** Fixed by a Fix headless run, or dismissed by hand. */
  state?: "fixed" | "dismissed";
  onDismiss(dismissed: boolean): void;
  /** The review is in a PR workspace, so findings can become drafts for GitHub. */
  canDraft: boolean;
}

/** A finding: its severity tag and title, then where it is, who raised it, and its id. Opens to
 * show the evidence, the fix, and the cross-review behind it. */
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
    state,
    onDismiss,
    canDraft,
  } = props;
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const [kept, setKept] = useState<"note" | "draft">();
  const placed = f.locations.map((loc) => (review ? place(review.files, loc) : undefined));
  const first = placed.find((p) => p);
  const notHere = review ? "Not in the open review" : "Open a review to see the code";
  const raisers = f.raisedBy?.length ? f.raisedBy : undefined;

  async function keep(draft: boolean) {
    if (!review || !first) return;
    const snippet = rowsText(review.files[first.file], first.changed);
    const text = findingText(f, source);
    if (await review.commentOn(first.file, first.changed, text, snippet, draft)) {
      setKept(draft ? "draft" : "note");
    }
  }

  const id = f.id?.includes("#") ? f.id : `#${f.id}`;
  const who = raisers
    ? raisers.map((r, i) => (
        <span key={r.persona} title={`${r.persona} rated it ${r.severity}`}>
          {i > 0 && ", "}
          {r.persona}
          {!r.inLens && " (off-lens)"}
        </span>
      ))
    : `${source}${f.offLens ? " (off-lens)" : ""}`;

  return (
    <div className={`finding${state ? " settled" : ""}`} data-finding={f.id}>
      <div className="finding-top">
        <input
          type="checkbox"
          className="finding-check"
          checked={checked}
          onChange={onCheck}
          title="Pick it to fix or send with others"
        />
        <SevTag severity={f.severity} />
        <button type="button" className="finding-head" onClick={onToggle} aria-expanded={open}>
          <Inline>{f.title}</Inline>
        </button>
        {state && (
          <span className={`pill${state === "fixed" ? " green" : ""}`}>
            {state === "fixed" ? "Fixed" : "Dismissed"}
          </span>
        )}
        <span className="finding-actions reveal">
          <IconButton
            icon="send"
            label="Send to terminal: paste it into a terminal's prompt"
            disabled={!review}
            onClick={() => onSend(findingText(f, source))}
          />
          <IconButton
            icon={kept === "note" ? "check" : "comments"}
            label={kept === "note" ? "Kept as note" : first ? "Keep as note on its code" : notHere}
            disabled={!first || !!kept}
            onClick={() => keep(false)}
          />
          {canDraft && (
            <IconButton
              icon={kept === "draft" ? "check" : "pr"}
              label={
                kept === "draft" ? "Drafted" : first ? "Draft PR comment on its code" : notHere
              }
              disabled={!first || !!kept}
              onClick={() => keep(true)}
            />
          )}
          {state !== "fixed" && (
            <IconButton
              icon={state === "dismissed" ? "retry" : "x"}
              label={state === "dismissed" ? "Bring it back" : "Dismiss"}
              onClick={() => onDismiss(state !== "dismissed")}
            />
          )}
        </span>
      </div>
      <div className="finding-meta">
        {f.locations.map((loc, i) => (
          <button
            type="button"
            className="finding-loc"
            // biome-ignore lint/suspicious/noArrayIndexKey: a finding's locations never reorder
            key={i}
            title={placed[i] ? "Show in the review" : "Open in the editor"}
            onClick={() => {
              const at = placed[i];
              if (at && review) review.jumpToRow(at.file, at.row);
              else openFile(workspace, loc.path, loc.start ?? undefined);
            }}
          >
            {locationText(loc)}
          </button>
        ))}
        <span className="finding-dot">·</span>
        <span className="finding-who">{who}</span>
        <span className="finding-dot">· {id}</span>
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
