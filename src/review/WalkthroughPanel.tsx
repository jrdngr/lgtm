import type { NoteDto } from "../bindings";
import { useWorkspace } from "../workspaces/context";
import { rowLabel } from "./comments";
import { type CurrentReview, useCurrentReview } from "./current";
import { CATEGORIES, CATEGORY_ORDER, READING_ORDER, tours } from "./notes";
import { activeTour, goToStep, setTourState, useTourState } from "./tour";
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
  if (!review) return <div className="placeholder">No review open</div>;
  const { notes } = review;
  const tour = activeTour(review.id, notes);
  const at = state.at[tour.id];
  const toggle = (c: NoteDto["category"]) =>
    setTourState(review.id, {
      hidden: state.hidden.includes(c) ? state.hidden.filter((h) => h !== c) : [...state.hidden, c],
    });
  return (
    <div className="comment-list note-list">
      <div className="tree-total">
        {run?.running ? (
          <>
            <span className="risk-note">{run.note}</span>
            <span className="spacer" />
            <button type="button" onClick={() => stopWalkthrough(review.id)}>
              Stop
            </button>
          </>
        ) : (
          <>
            <span className={run?.error ? "error" : undefined}>
              {run?.error ?? `${notes.length} ${notes.length === 1 ? "note" : "notes"}`}
            </span>
            <span className="spacer" />
            <button type="button" onClick={review.runWalkthrough} title="h in the review">
              Walkthrough
            </button>
          </>
        )}
      </div>
      {notes.length > 0 && (
        <div className="note-tools">
          <select
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
          {CATEGORY_ORDER.map((c) => (
            <button
              key={c}
              type="button"
              className={`note-filter${state.hidden.includes(c) ? "" : " on"}`}
              onClick={() => toggle(c)}
              title={`Show ${CATEGORIES[c].label.toLowerCase()} notes`}
            >
              {CATEGORIES[c].emoji}
            </button>
          ))}
          <label title="Show notes under their code in the review">
            <input
              type="checkbox"
              checked={state.inline}
              onChange={(e) => setTourState(review.id, { inline: e.target.checked })}
            />
            In review
          </label>
        </div>
      )}
      {notes.length === 0 && !run?.running && (
        <div className="placeholder">
          No walkthrough yet. Walkthrough (h) has an agent guide you through the change: what needs
          a careful look, how its flows run, and what to ask.
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
  );
}
