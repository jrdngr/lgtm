import { useState } from "react";
import type { Finding } from "../bindings";
import { openFile } from "../editor/open";
import { useCurrentReview } from "../review/current";
import { unified } from "../review/snippet";
import { contextMenu, copy } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { useWorkspace } from "../workspaces/context";
import { findingText, locationText, place } from "./findings";
import { Inline, Markdown } from "./Markdown";
import { SevTag } from "./Severity";

interface Props {
  finding: Finding;
  /** The reviewer it's from. */
  source: string;
  open: boolean;
  onToggle(): void;
  /** Picked to send with others. */
  checked: boolean;
  onCheck(): void;
  /** Pastes text into a terminal's prompt. */
  onSend(text: string): void;
  dismissed: boolean;
  onDismiss(dismissed: boolean): void;
  /** The review is in a PR workspace, so findings can become drafts for GitHub. */
  canDraft: boolean;
}

/** A finding: its severity tag and title, then where it is, who raised it, and its id. Opens to
 * show the evidence and the fix. */
export function FindingCard(props: Props) {
  const {
    finding: f,
    source,
    open,
    onToggle,
    checked,
    onCheck,
    onSend,
    dismissed,
    onDismiss,
    canDraft,
  } = props;
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const [kept, setKept] = useState<"note" | "draft">();
  const placed = f.locations.map((loc) => (review ? place(review.files, loc) : undefined));
  const first = placed.find((p) => p);
  const notHere = review ? "Not in the open review" : "Open a review to see the code";

  async function keep(draft: boolean) {
    if (!review || !first) return;
    const rows = review.files[first.file].rows;
    const snippet = unified(first.changed.map((r) => rows[r]));
    const text = findingText(f, source);
    if (await review.commentOn(first.file, first.changed, text, snippet, draft)) {
      setKept(draft ? "draft" : "note");
    }
  }

  function openLocation(i: number) {
    const at = placed[i];
    if (at && review) review.jumpToRow(at.file, at.row);
    else openFile(workspace, f.locations[i].path, f.locations[i].start ?? undefined);
  }

  const menu = contextMenu(() => [
    { label: "Open Location", disabled: !f.locations.length, onSelect: () => openLocation(0) },
    {
      label: "Send to Terminal",
      onSelect: () => onSend(findingText(f, source)),
    },
    { label: "Keep as Note", disabled: !first || !!kept, onSelect: () => keep(false) },
    canDraft && {
      label: "Draft PR Comment",
      disabled: !first || !!kept,
      onSelect: () => keep(true),
    },
    "-",
    { label: "Select for Batch", checked, onSelect: onCheck },
    { label: "Copy Finding", onSelect: () => copy(findingText(f, source)) },
    "-",
    { label: dismissed ? "Bring Back" : "Dismiss", onSelect: () => onDismiss(!dismissed) },
  ]);

  const id = f.id?.includes("#") ? f.id : `#${f.id}`;
  return (
    <div
      className={`finding${dismissed ? " settled" : ""}`}
      data-finding={f.id}
      onContextMenu={menu}
    >
      <div className="finding-top">
        <input type="checkbox" className="finding-check" checked={checked} onChange={onCheck} />
        <SevTag severity={f.severity} />
        <button type="button" className="finding-head" onClick={onToggle} aria-expanded={open}>
          <Inline>{f.title}</Inline>
        </button>
        {dismissed && <span className="pill">Dismissed</span>}
        <span className="finding-actions reveal">
          <IconButton
            icon="terminal"
            className="pipe-icon"
            label="Send to Terminal"
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
          <IconButton
            icon={dismissed ? "retry" : "x"}
            label={dismissed ? "Bring it back" : "Dismiss"}
            onClick={() => onDismiss(!dismissed)}
          />
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
            onClick={() => openLocation(i)}
          >
            {locationText(loc)}
          </button>
        ))}
        <span className="finding-dot">·</span>
        <span className="finding-who">{source}</span>
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
          {f.fix && <Markdown>{`**Fix:** ${f.fix}`}</Markdown>}
        </div>
      )}
    </div>
  );
}
