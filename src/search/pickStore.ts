import { EditorView } from "@codemirror/view";
import { useSyncExternalStore } from "react";
import type { WorkspaceName } from "../core/workspace";

export interface Pick {
  id: number;
  ws: WorkspaceName;
  /** Where focus was, to go back to on Escape. */
  from: Element | null;
  /** The editor focus was in, for `:N`. */
  view?: EditorView;
}

let current: Pick | undefined;
let picks = 0;
const listeners = new Set<() => void>();
const set = (p: Pick | undefined) => {
  current = p;
  for (const l of listeners) l();
};

export function openQuickPick(ws: WorkspaceName) {
  const from = document.activeElement;
  const el = from?.closest?.(".cm-editor");
  const view = el instanceof HTMLElement ? (EditorView.findFromDOM(el) ?? undefined) : undefined;
  set({ id: ++picks, ws, from, view });
}

export const closeQuickPick = () => set(undefined);

export function useQuickPick(): Pick | undefined {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => current,
  );
}

/** What the box asks for: files, `@` symbols, or `:N` a line in the editor. `file:N` is a file
 * at a line. */
export type Query =
  | { kind: "files"; text: string; line?: number }
  | { kind: "symbols"; text: string }
  | { kind: "line"; line: number };

export function parseQuery(q: string): Query {
  const t = q.trim();
  if (t.startsWith("@")) return { kind: "symbols", text: t.slice(1).trim() };
  const at = /^:(\d+)$/.exec(t);
  if (at) return { kind: "line", line: Number(at[1]) };
  const fileAt = /^(.*\S):(\d+)$/.exec(t);
  if (fileAt) return { kind: "files", text: fileAt[1], line: Number(fileAt[2]) };
  return { kind: "files", text: t };
}

export interface Match {
  score: number;
  /** Matched character indexes. */
  at: number[];
}

const boundary = (s: string, i: number) => i === 0 || /[/_\-. ]/.test(s[i - 1]);

/** Matches `query`'s characters in order in `text`, ignoring case. Higher scores are runs,
 * word starts, and matches in the file name. */
export function fuzzy(text: string, query: string): Match | undefined {
  if (!query) return { score: 0, at: [] };
  const t = text.toLowerCase();
  const q = query.toLowerCase().replace(/\s+/g, "");
  const nameStart = text.lastIndexOf("/") + 1;
  // Prefer a match inside the file name, else anywhere.
  const at = inOrder(t, q, nameStart) ?? inOrder(t, q, 0);
  if (!at) return undefined;
  let score = 0;
  for (let k = 0; k < at.length; k++) {
    const i = at[k];
    score += 1;
    if (k > 0 && at[k - 1] === i - 1) score += 3;
    if (boundary(text, i)) score += 2;
    if (i >= nameStart) score += 2;
  }
  return { score: score - text.length * 0.01, at };
}

/** The earliest indexes in `t` from `start` spelling `q`, preferring runs at word starts. */
function inOrder(t: string, q: string, start: number): number[] | undefined {
  const runStart = t.indexOf(q, start);
  if (runStart >= 0) return [...q].map((_, k) => runStart + k);
  const at: number[] = [];
  let i = start;
  for (const c of q) {
    i = t.indexOf(c, i);
    if (i < 0) return undefined;
    at.push(i++);
  }
  return at;
}
