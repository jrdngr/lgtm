import { Channel } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useSyncExternalStore } from "react";
import { endFrom, startRun } from "../agents/runs";
import { events } from "../bindings";
import { type CommandResult, commands } from "../ipc";
import { dockOf } from "../workbench/docks";
import { openQa } from "../workbench/panels";
import { report } from "../workspaces/activity";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";
import type { Session, Step } from "./model";

/** What the agent is doing for a session. `answer` is the update after decision `about`. */
export type BusyKind = "answer" | "discuss" | "reask";

export interface Busy {
  kind: BusyKind;
  /** The question it's about. */
  about?: string;
  /** The tool it's using now. */
  activity: string;
}

export interface QaState {
  sessions: Session[];
  loaded: boolean;
  /** The session the tab shows. Unset with `fresh` for a new one. */
  current?: string;
  /** The tab shows a new session's prompt. */
  fresh: boolean;
  /** The step the tab shows; the session's own step when unset. */
  view?: Step;
  /** A question to show next time Questions opens. */
  jump?: string;
  busy: Record<string, Busy>;
  /** Text the agent is writing, by `id:about`, until its run ends. */
  streams: Record<string, string>;
  error?: string;
}

const EMPTY: QaState = {
  sessions: [],
  loaded: false,
  fresh: false,
  busy: {},
  streams: {},
};
const states = new Map<string, QaState>();
const listeners = new Set<() => void>();
const watchers = new Map<string, { count: number; stop(): void }>();

const BUSY_NAMES: Record<BusyKind, string> = {
  answer: "Updating dependent questions",
  discuss: "Discussing",
  reask: "Rewriting the question",
};

export const busyName = (b: Busy) => BUSY_NAMES[b.kind];

export const getQa = (ws: WorkspaceName) => states.get(workspaceKey(ws)) ?? EMPTY;

function set(ws: WorkspaceName, f: (s: QaState) => QaState) {
  states.set(workspaceKey(ws), f(getQa(ws)));
  reportWaiting(ws);
  for (const l of listeners) l();
}

/** Sessions shown as waiting on the user in the workspace's row, by workspace key. */
const waiting = new Map<string, Set<string>>();

/** Shows each session a terminal agent asked as needing input until its answers go back. */
function reportWaiting(ws: WorkspaceName) {
  const key = workspaceKey(ws);
  const before = waiting.get(key) ?? new Set();
  const now = new Set<string>();
  for (const d of getQa(ws).sessions) {
    if (!d.fromTerminal || d.sent != null || d.archived) continue;
    now.add(d.id);
    report(ws, `qa:${d.id}`, "input", `Q&A: ${d.title}`);
  }
  for (const id of before) if (!now.has(id)) report(ws, `qa:${id}`, "gone", "");
  waiting.set(key, now);
}

export const setError = (ws: WorkspaceName, error?: string) => set(ws, (s) => ({ ...s, error }));

/** Shows session `id` in the tab, on the step it's at. */
export function selectSession(ws: WorkspaceName, id: string) {
  set(ws, (s) =>
    s.current === id && !s.fresh
      ? s
      : { ...s, current: id, fresh: false, view: undefined, error: undefined },
  );
}

/** Shows a new session's prompt in the tab. */
export const startFresh = (ws: WorkspaceName) =>
  set(ws, (s) => ({ ...s, fresh: true, view: undefined, error: undefined }));

export const setView = (ws: WorkspaceName, view: Step) => set(ws, (s) => ({ ...s, view }));

/** Opens Questions on question `id`. */
export const showQuestion = (ws: WorkspaceName, id: string) =>
  set(ws, (s) => ({ ...s, view: "questions", jump: id }));

/** Questions took the question it was asked to show. */
export const clearJump = (ws: WorkspaceName) =>
  set(ws, (s) => (s.jump ? { ...s, jump: undefined } : s));

/** Puts a session the backend returned in place of the old one. */
export function applySession(ws: WorkspaceName, d: Session) {
  set(ws, (s) => {
    const has = s.sessions.some((x) => x.id === d.id);
    const sessions = has ? s.sessions.map((x) => (x.id === d.id ? d : x)) : [...s.sessions, d];
    return { ...s, sessions, error: undefined };
  });
}

/** The session to show when none is picked: the newest active one. */
const fallback = (sessions: Session[]) =>
  [...sessions].reverse().find((d) => !d.archived)?.id ?? sessions.at(-1)?.id;

export function removeSession(ws: WorkspaceName, id: string) {
  set(ws, (s) => {
    const sessions = s.sessions.filter((x) => x.id !== id);
    if (s.current !== id) return { ...s, sessions };
    return { ...s, sessions, current: fallback(sessions), view: undefined };
  });
}

export async function loadSessions(ws: WorkspaceName) {
  const r = await commands.listQa(ws);
  if (r.status === "error") return set(ws, (s) => ({ ...s, loaded: true, error: r.error }));
  set(ws, (s) => {
    const current = r.data.some((d) => d.id === s.current) ? s.current : fallback(r.data);
    return { ...s, sessions: r.data, loaded: true, current };
  });
}

/** Applies a command's session, or shows its error. */
export async function act(ws: WorkspaceName, r: Promise<CommandResult<Session>>) {
  const got = await r;
  if (got.status === "error") setError(ws, got.error);
  else applySession(ws, got.data);
  return got;
}

interface RunOptions {
  kind: BusyKind;
  about?: string;
  /** What the Agent runs list says it's about. */
  subject: string;
  /** The channel carries the reply's text, kept in `streams`, rather than tool calls. */
  streams?: boolean;
}

/** Runs an agent step for session `id`, showing it as busy and in Agent runs. */
export async function runStep(
  ws: WorkspaceName,
  id: string,
  o: RunOptions,
  go: (channel: Channel<string>) => Promise<CommandResult<Session>>,
): Promise<CommandResult<Session>> {
  const key = `${id}:${o.about ?? ""}`;
  set(ws, (s) => ({
    ...s,
    error: undefined,
    busy: { ...s.busy, [id]: { kind: o.kind, about: o.about, activity: "" } },
    streams: o.streams ? { ...s.streams, [key]: "" } : s.streams,
  }));
  const handle = startRun(ws, {
    kind: "qa",
    subject: o.subject,
    stop: () => commands.stopQa(ws, id),
    retry: () => runStep(ws, id, o, go),
    open: () => showQa(ws, id),
  });
  const channel = new Channel<string>();
  channel.onmessage = (text) => {
    if (o.streams)
      set(ws, (s) => ({ ...s, streams: { ...s.streams, [key]: (s.streams[key] ?? "") + text } }));
    else {
      handle.activity(text);
      set(ws, (s) => {
        const b = s.busy[id];
        if (!b) return s;
        return { ...s, busy: { ...s.busy, [id]: { ...b, activity: text } } };
      });
    }
  };
  const r = await go(channel);
  endFrom(handle, r);
  set(ws, (s) => {
    const { [id]: _, ...busy } = s.busy;
    const { [key]: __, ...streams } = s.streams;
    return { ...s, busy, streams };
  });
  if (r.status === "error") setError(ws, r.error);
  else applySession(ws, r.data);
  return r;
}

function watch(ws: WorkspaceName) {
  const key = workspaceKey(ws);
  const w = watchers.get(key);
  if (w) {
    w.count++;
    return;
  }
  const sub = events.qaChanged(getCurrentWebviewWindow()).listen((e) => {
    if (workspaceKey(e.payload.workspace) === key) loadSessions(ws);
  });
  watchers.set(key, { count: 1, stop: () => sub.then((un) => un()) });
  loadSessions(ws);
}

function unwatch(ws: WorkspaceName) {
  const key = workspaceKey(ws);
  const w = watchers.get(key);
  if (!w) return;
  if (--w.count === 0) {
    w.stop();
    watchers.delete(key);
  }
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** The workspace's Q&A sessions, kept in step with terminal agents adding questions. */
export function useQa(ws: WorkspaceName): QaState {
  useEffect(() => {
    watch(ws);
    return () => unwatch(ws);
  }, [ws]);
  return useSyncExternalStore(subscribe, () => getQa(ws));
}

/** Opens the workspace's Q&A tab, or brings it forward, on session `id`. */
export function showQa(ws: WorkspaceName, id?: string) {
  if (id) selectSession(ws, id);
  const api = dockOf(ws);
  if (api) openQa(api);
}
