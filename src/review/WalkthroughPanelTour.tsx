import type { NoteDto } from "../bindings";
import { usePr } from "../github/pr";
import { type CtxEntry, contextMenu, copy } from "../ui/contextMenu";
import { Icon, IconButton } from "../ui/icons";
import { run } from "../ui/pathMenu";
import { useWorkspace } from "../workspaces/context";
import { rowLabel } from "./comments";
import type { CurrentReview } from "./current";
import { noteCommentText } from "./NoteBlock";
import { LevelDots, NoteIcon } from "./NoteIcon";
import { CATEGORIES, CATEGORY_ORDER, flowLabel, tours } from "./notes";
import { rowsText } from "./sendText";
import { activeTour, goToStep, setHidden, setTourState, stepTour, useTourState } from "./tour";

const SOURCE = { headless: "the walkthrough", terminal: "a terminal agent" } as const;

function showNote(review: CurrentReview, n: NoteDto) {
  if (n.rows) review.jumpToRow(n.file, n.rows.start);
  else review.jump(n.file);
}

/** Keeps note `n` as one of yours on its changed rows, or with `draft`, as a draft for the PR. */
function keep(review: CurrentReview, n: NoteDto, draft: boolean) {
  const f = review.files[n.file];
  if (!f || !n.rows) return;
  const rows: number[] = [];
  for (let r = n.rows.start; r <= n.rows.end; r++) if (f.rows[r]?.kind !== "context") rows.push(r);
  run(review.commentOn(n.file, rows, noteCommentText(n), rowsText(f, rows), draft));
}

/** The tour card with its stepper, the category chips, and the tour's notes as a numbered
 * path: visited notes dimmed, the current one ringed. */
export function WalkthroughTour({ review }: { review: CurrentReview }) {
  const state = useTourState(review.id);
  const canDraft = !!usePr(useWorkspace());
  const { notes } = review;
  const all = tours(notes);
  const tour = activeTour(review.id, notes);
  const at = state.at[tour.id];
  const step = (dir: 1 | -1) => {
    const note = stepTour(review.id, notes, dir);
    if (note) showNote(review, note);
  };
  const toggle = (c: NoteDto["category"]) =>
    setHidden(
      review.id,
      notes,
      state.hidden.includes(c) ? state.hidden.filter((h) => h !== c) : [...state.hidden, c],
    );
  const counts = new Map<NoteDto["category"], number>();
  for (const n of all.find((t) => t.id === tour.id)?.notes ?? []) {
    counts.set(n.category, (counts.get(n.category) ?? 0) + 1);
  }
  const length = tour.notes.length;
  const noteMenu = (n: NoteDto, i: number): CtxEntry[] => [
    {
      label: "Jump to Note",
      onSelect: () => {
        const note = goToStep(review.id, notes, tour.id, i);
        if (note) showNote(review, note);
      },
    },
    { label: "Keep as Note", disabled: !n.rows, onSelect: () => keep(review, n, false) },
    canDraft && {
      label: "Draft PR Comment",
      disabled: !n.rows,
      onSelect: () => keep(review, n, true),
    },
    { label: "Copy Text", onSelect: () => copy(noteCommentText(n)) },
    "-",
    { label: "Hide This Category", onSelect: () => toggle(n.category) },
  ];
  return (
    <>
      <div className="note-tour">
        <div className="note-tour-top">
          <span className="note-icon note-icon-flow">
            <Icon name="compass" size={14} />
          </span>
          <span className="note-tour-name">{tour.name}</span>
          <select
            value={tour.id}
            onChange={(e) => setTourState(review.id, { active: e.target.value })}
            title="The tour n and N step through"
            aria-label="Tour"
          >
            {all.map((t) => (
              <option key={t.id} value={t.id}>
                {t.name} ({t.notes.length})
              </option>
            ))}
          </select>
          <span className="note-stepper">
            <IconButton
              icon="chevronLeft"
              label="Previous note (N)"
              disabled={length === 0}
              onClick={() => step(-1)}
            />
            <span>
              {at === undefined ? "–" : at + 1} / {length}
            </span>
            <IconButton
              icon="chevronRight"
              label="Next note (n)"
              disabled={length === 0}
              onClick={() => step(1)}
            />
          </span>
          <span className="note-tour-caret">
            <Icon name="chevronDown" size={12} />
          </span>
        </div>
        <div className="note-tour-progress">
          <b style={{ width: `${length && at !== undefined ? ((at + 1) / length) * 100 : 0}%` }} />
        </div>
      </div>
      <div className="note-chips">
        {CATEGORY_ORDER.filter((c) => counts.has(c)).map((c) => {
          const on = !state.hidden.includes(c);
          return (
            <button
              key={c}
              type="button"
              className={`note-chip${on ? "" : " off"}`}
              aria-pressed={on}
              onClick={() => toggle(c)}
              title={`${on ? "Hide" : "Show"} ${CATEGORIES[c].label.toLowerCase()} notes`}
            >
              <NoteIcon category={c} level={c === "important" ? "high" : undefined} />
              {counts.get(c)}
            </button>
          );
        })}
      </div>
      <ol className="note-path">
        {tour.notes.map((n, i) => {
          const flow = flowLabel(n, notes);
          return (
            <li
              key={n.id}
              className={i === at ? "current" : at !== undefined && i < at ? "seen" : undefined}
            >
              <button
                type="button"
                onClick={() => {
                  const note = goToStep(review.id, notes, tour.id, i);
                  if (note) showNote(review, note);
                }}
                onContextMenu={contextMenu(() => noteMenu(n, i))}
                title={`${n.path}, from ${SOURCE[n.source]}`}
              >
                <span className="note-dot">
                  <NoteIcon category={n.category} level={n.level} />
                </span>
                <span className="note-path-text">
                  <span className="note-path-title">{n.title}</span>
                  <span className="note-path-where">
                    {n.category === "important" && n.level && <LevelDots level={n.level} />}
                    {flow && <span className="pill">{flow}</span>}
                    {rowLabel(review.files, n.file, n.path, n.rows?.start)}
                    {!n.rows && <span className="pill amber">code changed</span>}
                  </span>
                </span>
              </button>
            </li>
          );
        })}
      </ol>
    </>
  );
}
