import type { NoteDto } from "../bindings";
import { IconButton } from "../ui/icons";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { rowLabel } from "./comments";
import { type CurrentReview, useCurrentReview } from "./current";
import { CATEGORIES, CATEGORY_ORDER, READING_ORDER, tours } from "./notes";
import { activeTour, goToStep, setTourState, useTourState } from "./tour";
import { WalkthroughPrompt } from "./WalkthroughPrompt";
import { stopWalkthrough, useWalkthroughRun } from "./walkthroughRun";
import "./comments.css";
import "./notes.css";

const SOURCE = { headless: "the walkthrough", terminal: "a terminal agent" } as const;

export function showNote(review: CurrentReview, n: NoteDto) {
  if (n.rows) review.jumpToRow(n.file, n.rows.start);
  else review.jump(n.file);
}

/** The current review's walkthrough: its notes along the chosen tour, filtered by category.
 * Click a note to go to it; `n` and `N` in the review step through the tour. */
export function WalkthroughPanel() {
  const review = useCurrentReview(useWorkspace());
  const run = useWalkthroughRun(review?.id);
  const state = useTourState(review?.id);
  if (!review) {
    return (
      <div className="side-split">
        <SectionHeader title="Walkthrough" />
        <Empty>No review open</Empty>
      </div>
    );
  }
  const { notes } = review;
  const tour = activeTour(review.id, notes);
  const at = state.at[tour.id];
  const toggle = (c: NoteDto["category"]) =>
    setTourState(review.id, {
      hidden: state.hidden.includes(c) ? state.hidden.filter((h) => h !== c) : [...state.hidden, c],
    });
  const counts = new Map<NoteDto["category"], number>();
  for (const n of tours(notes).find((t) => t.id === tour.id)?.notes ?? []) {
    counts.set(n.category, (counts.get(n.category) ?? 0) + 1);
  }
  return (
    <div className="side-split">
      <SectionHeader
        title="Walkthrough"
        meta={notes.length > 0 && `${notes.length} ${notes.length === 1 ? "note" : "notes"}`}
      >
        {notes.length > 0 && (
          <IconButton
            icon={state.inline ? "eye" : "eyeOff"}
            label={state.inline ? "Hide notes in the review" : "Show notes in the review"}
            aria-pressed={state.inline}
            onClick={() => setTourState(review.id, { inline: !state.inline })}
          />
        )}
        {run?.running ? (
          <IconButton
            icon="x"
            label="Stop the walkthrough"
            onClick={() => stopWalkthrough(review.id)}
          />
        ) : (
          <IconButton
            icon="sparkle"
            label="Write a walkthrough (h)"
            onClick={review.runWalkthrough}
          />
        )}
      </SectionHeader>
      <div className="side-split-body comment-list note-list">
        {(run?.running || run?.error) && (
          <div className={`note-status${run.error && !run.running ? " error" : ""}`}>
            {run.running ? run.note : run.error}
          </div>
        )}
        <WalkthroughPrompt />
        {notes.length > 0 && (
          <div className="note-tools">
            <select
              className="note-tour"
              value={tour.id}
              onChange={(e) => setTourState(review.id, { active: e.target.value })}
              title="The tour n and N step through"
            >
              {tours(notes).map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name} ({t.notes.length})
                </option>
              ))}
            </select>
            {CATEGORY_ORDER.filter((c) => counts.has(c)).map((c) => {
              const on = !state.hidden.includes(c);
              return (
                <button
                  key={c}
                  type="button"
                  className={`note-filter${on ? " on" : ""}`}
                  aria-pressed={on}
                  onClick={() => toggle(c)}
                  title={`${on ? "Hide" : "Show"} ${CATEGORIES[c].label.toLowerCase()} notes`}
                >
                  {CATEGORIES[c].emoji} {counts.get(c)}
                </button>
              );
            })}
          </div>
        )}
        {notes.length === 0 && !run?.running && (
          <div className="placeholder">
            No walkthrough yet. Walkthrough (h) has an agent guide you through the change: what
            needs a careful look, how its flows run, and what to ask.
          </div>
        )}
        <ul>
          {tour.notes.map((n, i) => (
            <li key={n.id} className={i === at ? "current" : undefined}>
              <button
                type="button"
                onClick={() => {
                  const note = goToStep(review.id, notes, tour.id, i);
                  if (note) showNote(review, note);
                }}
                title={`${n.path}, from ${SOURCE[n.source]}`}
              >
                <span className="comment-where">
                  <span className="note-emoji">{CATEGORIES[n.category].emoji}</span>
                  {n.flow && tour.id !== READING_ORDER ? `${n.flow.step}. ` : ""}
                  {rowLabel(review.files, n.file, n.path, n.rows?.start)}
                  {!n.rows && <span className="comment-outdated">code changed</span>}
                </span>
                <span className="comment-preview">{n.title}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
