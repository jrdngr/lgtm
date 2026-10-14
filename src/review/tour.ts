import { useSyncExternalStore } from "react";
import type { Category, NoteDto } from "../bindings";
import { nextStep, READING_ORDER, tours } from "./notes";

/** Where the reviewer is in a review's walkthrough: the tour they're on, their step in each
 * tour, and what the notes show. */
export interface TourState {
  active: string;
  /** Step index by tour id. */
  at: Record<string, number>;
  /** Categories left out of the panel and the review. */
  hidden: Category[];
  /** Note blocks show in the review. */
  inline: boolean;
}

const START: TourState = { active: READING_ORDER, at: {}, hidden: [], inline: true };

const states = new Map<number, TourState>();
const listeners = new Set<() => void>();

export function tourState(review: number): TourState {
  return states.get(review) ?? START;
}

export function setTourState(review: number, change: Partial<TourState>) {
  states.set(review, { ...tourState(review), ...change });
  for (const l of listeners) l();
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useTourState(review: number | undefined): TourState {
  return useSyncExternalStore(subscribe, () => (review === undefined ? START : tourState(review)));
}

/** The notes of the review's active tour that aren't hidden, or the reading order when that
 * tour is gone. */
export function activeTour(review: number, notes: NoteDto[]) {
  const s = tourState(review);
  const all = tours(notes);
  const tour = all.find((t) => t.id === s.active) ?? all[0];
  return { ...tour, notes: tour.notes.filter((n) => !s.hidden.includes(n.category)) };
}

/** Hides `hidden`, keeping each tour on its note. A tour whose note gets hidden loses its
 * place. */
export function setHidden(review: number, notes: NoteDto[], hidden: Category[]) {
  const s = tourState(review);
  const shown = (h: Category[]) => (n: NoteDto) => !h.includes(n.category);
  const at = { ...s.at };
  for (const t of tours(notes)) {
    if (at[t.id] === undefined) continue;
    const note = t.notes.filter(shown(s.hidden))[at[t.id]];
    const i = note ? t.notes.filter(shown(hidden)).indexOf(note) : -1;
    if (i < 0) delete at[t.id];
    else at[t.id] = i;
  }
  setTourState(review, { hidden, at });
}

/** Moves to step `index` of tour `id` and returns its note. */
export function goToStep(review: number, notes: NoteDto[], id: string, index: number) {
  const s = tourState(review);
  setTourState(review, { active: id, at: { ...s.at, [id]: index } });
  return activeTour(review, notes).notes[index];
}

/** Moves one step forward or back in the active tour and returns that note. */
export function stepTour(review: number, notes: NoteDto[], dir: 1 | -1): NoteDto | undefined {
  const tour = activeTour(review, notes);
  const at = nextStep(tour.notes.length, tourState(review).at[tour.id], dir);
  return at === undefined ? undefined : goToStep(review, notes, tour.id, at);
}

/** The note the active tour is on. */
export function currentNote(review: number, notes: NoteDto[]): NoteDto | undefined {
  const tour = activeTour(review, notes);
  const at = tourState(review).at[tour.id];
  return at === undefined ? undefined : tour.notes[at];
}
