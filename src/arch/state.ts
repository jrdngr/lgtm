import { useEffect, useState, useSyncExternalStore } from "react";
import type { ArchDto, ArchRange } from "../bindings";
import { commands, errorText, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";

/** One review's view of its architecture, shared by the sidebar section and the tab. */
export interface ArchView {
  transitive: boolean;
  /** Commits picked in History, instead of the review's base and head. */
  range?: ArchRange;
  /** Bumped to load again after a check or an assessment. */
  version: number;
}

const views = new Map<number, ArchView>();
const listeners = new Set<() => void>();
const EMPTY: ArchView = { transitive: false, version: 0 };

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function archView(review: number): ArchView {
  return views.get(review) ?? EMPTY;
}

export function setArchView(review: number, change: Partial<ArchView>) {
  views.set(review, { ...archView(review), ...change });
  for (const l of listeners) l();
}

/** Loads the review's architecture again, after a check or an assessment. */
export function reloadArch(review: number) {
  setArchView(review, { version: archView(review).version + 1 });
}

export function useArchView(review: number | undefined): ArchView {
  return useSyncExternalStore(subscribe, () => (review === undefined ? EMPTY : archView(review)));
}

interface Load {
  id: number;
  files: object;
  p: Promise<ArchDto>;
}

/** Loads by files, view and version, so the panels showing one review share a load. Keeps
 * each review's latest, for the few reviews seen last. */
const loads = new Map<string, Load>();
const KEEP = 8;

function load(id: number, files: object, view: ArchView) {
  const key = JSON.stringify([id, view.transitive, view.range ?? null, view.version]);
  let l = loads.get(key);
  if (!l || l.files !== files) {
    const p = unwrap(commands.reviewArch(id, view.transitive, view.range ?? null));
    const made: Load = { id, files, p };
    l = made;
    for (const [k, old] of loads) if (old.id === id) loads.delete(k);
    loads.set(key, made);
    while (loads.size > KEEP) loads.delete(loads.keys().next().value as string);
    p.catch(() => loads.get(key) === made && loads.delete(key));
  }
  return l.p;
}

interface State {
  id?: number;
  arch?: ArchDto;
  error?: string;
}

/** The current review's architecture diff, as its view asks for. Keeps the review's last one
 * while the next loads. */
export function useArch(review: CurrentReview | undefined) {
  const id = review?.id;
  const files = review?.files;
  const view = useArchView(id);
  const [state, setState] = useState<State>({});
  useEffect(() => {
    if (id === undefined || !files) return;
    let live = true;
    load(id, files, view).then(
      (arch) => live && setState({ id, arch }),
      (e) => live && setState({ id, error: errorText(e) }),
    );
    return () => {
      live = false;
    };
  }, [id, files, view]);
  const mine = state.id === id;
  return { arch: mine ? state.arch : undefined, error: mine ? state.error : undefined, view };
}
