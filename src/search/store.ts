import { useSyncExternalStore } from "react";
import type { Hit, TextQuery } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";

export type SearchMode = "references" | "text";

export interface RefsState {
  name: string;
  hits?: Hit[];
  truncated?: boolean;
  error?: string;
}

/** Where text search looks. */
export type Scope = "workspace" | "changed" | "tabs";

export interface TextState {
  query: TextQuery;
  scope: Scope;
  replace: string;
  /** Bumped to make the view search again with `query`, and focus its box. */
  asked: number;
}

interface State {
  mode: SearchMode;
  refs?: RefsState;
  text: TextState;
}

const blank = (): State => ({
  mode: "references",
  text: {
    query: {
      pattern: "",
      regex: false,
      caseSensitive: false,
      wholeWord: false,
      include: "",
      exclude: "",
      paths: null,
    },
    scope: "workspace",
    replace: "",
    asked: 0,
  },
});

const states = new Map<string, State>();
const listeners = new Set<() => void>();
const showListeners = new Set<() => void>();

function get(ws: WorkspaceName): State {
  const key = workspaceKey(ws);
  let s = states.get(key);
  if (!s) {
    s = blank();
    states.set(key, s);
  }
  return s;
}

function set(ws: WorkspaceName, f: (s: State) => State) {
  states.set(workspaceKey(ws), f(get(ws)));
  for (const l of listeners) l();
}

export function useSearch(ws: WorkspaceName): State {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => get(ws),
  );
}

export const setMode = (ws: WorkspaceName, mode: SearchMode) => set(ws, (s) => ({ ...s, mode }));

export const setText = (ws: WorkspaceName, f: (t: TextState) => TextState) =>
  set(ws, (s) => ({ ...s, text: f(s.text) }));

/** Calls `l` when something asks for the Search tab. */
export function onShowSearch(l: () => void): () => void {
  showListeners.add(l);
  return () => showListeners.delete(l);
}

function show() {
  for (const l of showListeners) l();
}

/** Lists where `name` is used, in the Search tab. */
export async function findReferences(ws: WorkspaceName, name: string) {
  set(ws, (s) => ({ ...s, mode: "references", refs: { name } }));
  show();
  const refs: RefsState = await unwrap(commands.findReferences(ws, name)).then(
    (r) => ({ name, hits: r.hits, truncated: r.truncated }),
    (e) => ({ name, error: String(e) }),
  );
  set(ws, (s) => (s.refs?.name === name ? { ...s, refs } : s));
}

/** Opens `text` in the Search tab's text search. */
export function searchWorkspace(ws: WorkspaceName, text: string) {
  setText(ws, (t) => ({
    ...t,
    query: text ? { ...t.query, pattern: text, regex: false } : t.query,
    asked: t.asked + 1,
  }));
  setMode(ws, "text");
  show();
}

/** Opens `name`'s definition, or lists them when there are several or none. */
export async function goToDefinition(ws: WorkspaceName, name: string) {
  const hits = await unwrap(commands.findReferences(ws, name)).catch(() => undefined);
  const defs = hits?.hits.filter((h) => h.def) ?? [];
  if (defs.length === 1) openFile(ws, defs[0].path, defs[0].line);
  else findReferences(ws, name);
}
