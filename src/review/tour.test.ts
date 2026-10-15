import { expect, test } from "vitest";
import type { NoteDto } from "../bindings";
import { currentNote, setHidden, setTourState, stepTour, tourState } from "./tour";

const note = (id: number, extra: Partial<NoteDto> = {}): NoteDto => ({
  id,
  path: "a.rs",
  file: 0,
  rows: { start: id, end: id },
  category: "context",
  level: null,
  title: "",
  body: "",
  flow: null,
  position: id,
  source: "headless",
  ...extra,
});

test("each tour keeps its own place, and hidden categories are skipped", () => {
  const flow = (step: number) => ({ category: "flow" as const, flow: { flow: "Save", step } });
  const notes = [note(0), note(1, flow(2)), note(2, { category: "question" }), note(3, flow(1))];
  const review = 7;

  expect(stepTour(review, notes, 1)?.id).toBe(0);
  expect(stepTour(review, notes, 1)?.id).toBe(1);

  setTourState(review, { active: "flow:Save" });
  expect(stepTour(review, notes, 1)?.id).toBe(3);
  expect(stepTour(review, notes, 1)?.id).toBe(1);

  setTourState(review, { active: "reading", hidden: ["question"] });
  expect(currentNote(review, notes)?.id).toBe(1);
  expect(stepTour(review, notes, 1)?.id).toBe(3);
});

test("hiding a category keeps the tour on its note, or drops its place when that's hidden", () => {
  const notes = [note(0, { category: "question" }), note(1), note(2, { category: "question" })];
  const review = 8;
  setHidden(review, notes, []);
  stepTour(review, notes, 1);
  stepTour(review, notes, 1);

  setHidden(review, notes, ["question"]);
  expect(currentNote(review, notes)?.id).toBe(1);
  setHidden(review, notes, []);
  expect(currentNote(review, notes)?.id).toBe(1);

  stepTour(review, notes, 1);
  setHidden(review, notes, ["question"]);
  expect(currentNote(review, notes)).toBeUndefined();
});

test("hidden categories and inline notes carry over to every review", () => {
  setTourState(9, { hidden: ["context"], inline: false });
  expect(tourState(10)).toMatchObject({ hidden: ["context"], inline: false });
  setTourState(10, { hidden: [], inline: true });
  expect(tourState(9)).toMatchObject({ hidden: [], inline: true });
});
