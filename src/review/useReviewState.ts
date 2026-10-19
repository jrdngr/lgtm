import { useMemo } from "react";
import type { ReviewDto, StateDto } from "../bindings";
import type { DoneFn } from "./marks";
import { flatten } from "./model";
import { fileProgress, indexState, rowStates } from "./state";

const NO_STATE: StateDto = {
  marks: [],
  checks: [],
  amber: [],
  comments: [],
  threads: [],
};

/** A review's files, and the state and progress of their rows. `done` says which marks count
 * toward progress. */
export function useReviewState(review: ReviewDto | undefined, done: DoneFn) {
  // biome-ignore lint/correctness/useExhaustiveDependencies: marking replaces only `state`
  const files = useMemo(() => (review ? flatten(review) : []), [review?.files]);
  const index = useMemo(() => indexState(review?.state ?? NO_STATE), [review?.state]);
  const rowState = useMemo(() => rowStates(files, index), [files, index]);
  const progress = useMemo(() => fileProgress(files, rowState, done), [files, rowState, done]);
  return { files, index, rowState, progress };
}
