import { useEffect, useRef, useState } from "react";
import type { HeadDto, ReviewDto, StateDto } from "../bindings";
import { commands } from "../ipc";
import { useGithubOff } from "../tools/agents";
import type { WorkspaceName } from "../workspaces/context";
import { reloadPr, usePr } from "./pr";

interface Deps {
  onState(state: StateDto): void;
  onError(message: string): void;
  /** Loads a revset as the review. Resolves to the error message when it fails. */
  load(revset: string): Promise<string | undefined>;
}

/** A review in a PR workspace: pulls the PR's threads each time the whole PR opens, checks
 * whether the PR moved on GitHub, and fetches it again. */
export function usePrReview(
  workspace: WorkspaceName,
  review: ReviewDto | undefined,
  { onState, onError, load }: Deps,
) {
  const pr = usePr(workspace);
  const off = useGithubOff();
  const [head, setHead] = useState<HeadDto>();
  /** What's running, for the note. */
  const [busy, setBusy] = useState<string>();
  const refetching = useRef(false);
  const pulledFor = useRef<number>(undefined);
  // GitHub's lines match the review's rows only in the whole PR's Combined view.
  const whole = !!review && !!pr && review.revset === pr.revset && review.view.kind === "combined";
  const id = whole ? review.id : undefined;

  /** Pulls the PR's threads into the review. `quiet` skips the error when it can't. */
  async function pull(quiet = false) {
    if (id === undefined) {
      if (!quiet && pr) onError(`Open the whole PR (${pr.revset}) in the Combined view to pull.`);
      return;
    }
    setBusy("Pulling comments");
    const r = await commands.pullPrComments(id);
    setBusy(undefined);
    if (r.status === "error") onError(`Couldn't pull the PR's comments: ${r.error}`);
    else onState(r.data);
  }

  async function checkHead() {
    const r = await commands.prHead(workspace);
    if (r.status === "ok") setHead(r.data);
  }

  /** Fetches the PR, then opens it again, which pulls its threads into the new review. */
  async function refetch() {
    if (refetching.current) return;
    refetching.current = true;
    setBusy("Fetching the PR");
    const r = await commands.refetchPr(workspace, id ?? null);
    setBusy(undefined);
    refetching.current = false;
    if (r.status === "error") return onError(r.error);
    setHead(undefined);
    const next = await reloadPr(workspace);
    if (next) await load(next.revset);
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: once per opened review
  useEffect(() => {
    if (off || id === undefined || pulledFor.current === id) return;
    pulledFor.current = id;
    pull();
    checkHead();
  }, [off, id]);

  return { pr: off ? null : pr, whole, head, busy, refetch, pull };
}

export type PrReview = ReturnType<typeof usePrReview>;
