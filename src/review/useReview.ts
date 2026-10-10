import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef, useState } from "react";
import { events, type ReviewDto, type StateDto, type View } from "../bindings";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import { type Moves, movesFrom } from "./follow";

export interface Loaded {
  review: ReviewDto;
  /** Where rows went since the previous `review`, when this one came from a refresh. */
  moves?: Moves;
}

/** Loads a review in a workspace, keeps it live as the repo changes, and tracks a
 * stale working copy. */
export function useReview(workspace: WorkspaceName) {
  const [loaded, setLoaded] = useState<Loaded>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const [stale, setStale] = useState<string | null>(null);
  const current = useRef<ReviewDto>(undefined);
  const busy = useRef(false);
  const opened = useRef<string>(undefined);
  const again = useRef(false);

  function show(next: Loaded) {
    current.current = next.review;
    setLoaded(next);
    setStale(next.review.stale);
  }

  /** Loading the revset this pane already loaded continues its review session. Returns the
   * error message when it fails. */
  async function open(revset: string): Promise<ReviewDto | string> {
    const resume = opened.current === revset;
    opened.current = revset;
    setLoading(true);
    setError(undefined);
    const r = await commands.openReview(workspace, revset, resume);
    setLoading(false);
    if (r.status === "error") {
      setError(r.error);
      return r.error;
    }
    if (current.current) commands.closeReview(current.current.id);
    show({ review: r.data });
    return r.data;
  }

  /** Reloads the review, in `view` if given. Repeated calls while one runs coalesce. */
  async function refresh(view?: View) {
    const before = current.current;
    if (!before) return;
    if (busy.current && !view) {
      again.current = true;
      return;
    }
    busy.current = true;
    const r = await commands.refreshReview(before.id, view ?? null);
    busy.current = false;
    if (r.status === "error") setError(r.error);
    else {
      setError(undefined);
      const old = new Map(before.files.map((f, i) => [f.path, i]));
      const files: [number, number][] = [];
      r.data.review.files.forEach((f, i) => {
        const was = old.get(f.path) ?? (f.oldPath === null ? undefined : old.get(f.oldPath));
        if (was !== undefined) files.push([was, i]);
      });
      show({ review: r.data.review, moves: movesFrom(r.data.moves, files) });
    }
    if (again.current) {
      again.current = false;
      refresh();
    }
  }

  function setState(state: StateDto) {
    setLoaded((l) => {
      if (!l) return l;
      const review = { ...l.review, state };
      current.current = review;
      return { review };
    });
  }

  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;

  useEffect(() => {
    // Scoped to this window: other windows hold other repos.
    const win = getCurrentWebviewWindow();
    const changed = events.repoChanged(win).listen(() => refreshRef.current());
    const staleSub = events.workingCopyStale(win).listen((e) => {
      if (e.payload.workspace === workspace) setStale(e.payload.message);
    });
    return () => {
      changed.then((un) => un());
      staleSub.then((un) => un());
      if (current.current) commands.closeReview(current.current.id);
    };
  }, [workspace]);

  return { loaded, loading, error, setError, stale, open, refresh, setState };
}
