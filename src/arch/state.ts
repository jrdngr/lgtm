import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import type { ArchDto, ArchRange } from "../bindings";
import { commands, errorText, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";

/** One review's view of its architecture, shared by the sidebar section and the tab. */
export interface ArchView {
  transitive: boolean;
  /** Commits picked in History, instead of the review's base and head. */
  range?: ArchRange;
  /** Bumped to load again after an assessment. */
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

/** Loads the review's architecture again, after an assessment. */
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

/** The commits a review shows, so another review of the same ones (a review pane taking over
 * from the idle review) can show the last architecture while its own loads. */
const contentOf = (review: CurrentReview | undefined, view: ArchView) =>
  review?.revset && review.head
    ? JSON.stringify([review.revset, review.head, view.transitive, view.range ?? null])
    : undefined;
const lastByContent = new Map<string, ArchDto>();

/** Look checks made here and not yet in a load, by review content and then Look key. */
const checks = new Map<string, Map<string, boolean>>();
const checkListeners = new Set<() => void>();

function subscribeChecks(l: () => void) {
  checkListeners.add(l);
  return () => checkListeners.delete(l);
}

const checkKey = (review: CurrentReview) => `${review.revset}@${review.head}`;

/** Checks or unchecks a Look item at once, saving it in the background. Resolves to the
 * error when saving fails, and the check goes back. */
export async function checkLook(
  review: CurrentReview,
  key: string,
  checked: boolean,
): Promise<string | undefined> {
  const k = checkKey(review);
  const set = (on: boolean | undefined) => {
    const next = new Map(checks.get(k));
    if (on === undefined) next.delete(key);
    else next.set(key, on);
    checks.set(k, next);
    for (const l of checkListeners) l();
  };
  set(checked);
  const r = await commands.checkLook(review.id, key, checked);
  if (r.status === "ok") return undefined;
  set(undefined);
  return r.error;
}

function withChecks(arch: ArchDto, mine: Map<string, boolean> | undefined): ArchDto {
  if (!mine?.size) return arch;
  return {
    ...arch,
    look: arch.look.map((l) => (mine.has(l.key) ? { ...l, checked: !!mine.get(l.key) } : l)),
  };
}

/** The current review's architecture diff, as its view asks for, with the checks made here.
 * Keeps the review's last one while the next loads, and shows one of the same commits while a
 * new review of them loads. */
export function useArch(review: CurrentReview | undefined) {
  const id = review?.id;
  const files = review?.files;
  const view = useArchView(id);
  const content = contentOf(review, view);
  const [state, setState] = useState<State>({});
  const mineChecks = useSyncExternalStore(subscribeChecks, () =>
    review ? checks.get(checkKey(review)) : undefined,
  );
  useEffect(() => {
    if (id === undefined || !files) return;
    let live = true;
    load(id, files, view).then(
      (arch) => {
        if (content) lastByContent.set(content, arch);
        if (live) setState({ id, arch });
      },
      (e) => live && setState({ id, error: errorText(e) }),
    );
    return () => {
      live = false;
    };
  }, [id, files, view, content]);
  const mine = state.id === id;
  const arch = mine ? state.arch : content ? lastByContent.get(content) : undefined;
  const checked = useMemo(() => arch && withChecks(arch, mineChecks), [arch, mineChecks]);
  return { arch: checked, error: mine ? state.error : undefined, view };
}
