import { expect, test } from "vitest";
import type { NoteDto } from "../bindings";
import { currentNote, setTourState, stepTour } from "./tour";

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
