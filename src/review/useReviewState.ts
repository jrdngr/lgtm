import { useMemo } from "react";
import type { ReviewDto, StateDto } from "../bindings";
import { flatten } from "./model";
import { fileProgress, indexState, rowStates } from "./state";

const NO_STATE: StateDto = { marks: [], checks: [], amber: [], comments: [], notes: [] };

/** A review's files, and the state and progress of their rows. */
export function useReviewState(review: ReviewDto | undefined) {
  // biome-ignore lint/correctness/useExhaustiveDependencies: marking replaces only `state`
  const files = useMemo(() => (review ? flatten(review) : []), [review?.files]);
  const index = useMemo(() => indexState(review?.state ?? NO_STATE), [review?.state]);
  const rowState = useMemo(() => rowStates(files, index), [files, index]);
  const progress = useMemo(() => fileProgress(files, rowState), [files, rowState]);
  return { files, index, rowState, progress };
}
