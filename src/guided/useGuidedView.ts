import { useMemo } from "react";
import type { CurrentReview } from "../review/current";
import { type GuidedView, guidedView } from "./model";
import { useGuided } from "./store";

/** The current review's guided review, laid over its files and comments. */
export function useGuidedView(review: CurrentReview | undefined) {
  const { g, error } = useGuided(review?.id);
  const files = review?.files;
  const comments = review?.comments;
  const view = useMemo<GuidedView | undefined>(
    () => (g && files && comments ? guidedView(files, comments, g) : undefined),
    [g, files, comments],
  );
  return { g, error, view };
}
