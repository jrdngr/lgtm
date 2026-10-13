import { Channel } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useSyncExternalStore } from "react";
import { endFrom, startRun } from "../agents/runs";
import { events } from "../bindings";
import { type CommandResult, commands } from "../ipc";
import { dockOf } from "../workbench/docks";
import { openDesign } from "../workbench/panels";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";
import type { Design, View } from "./model";

/** What the agent is doing for a design. `answer` is the update after decision `about`. */
export type BusyKind =
  | "draft"
  | "answer"
  | "discuss"
  | "reask"
  | "revise"
  | "comments"
  | "ask"
  | "artifact"
  | "html";

export interface Busy {
  kind: BusyKind;
  /** The question, comment or file it's about. */
  about?: string;
  /** The tool it's using now. */
  activity: string;
}

export interface DesignsState {
  designs: Design[];
  loaded: boolean;
  /** The design the tab and panel show. */
  current?: string;
  /** What the Design tab shows; its step's view when unset. */
  view?: View;
  /** The file the Docs view shows, relative to the design's folder. */
  doc?: string;
  busy: Record<string, Busy>;
  /** Text the agent is writing, by `slug:about`, until its run ends. */
  streams: Record<string, string>;
  error?: string;
}

const EMPTY: DesignsState = { designs: [], loaded: false, busy: {}, streams: {} };
const states = new Map<string, DesignsState>();
const listeners = new Set<() => void>();
const watchers = new Map<string, { count: number; stop(): void }>();

const BUSY_NAMES: Record<BusyKind, string> = {
  draft: "Drafting the design",
  answer: "Updating the open questions",
  discuss: "Discussing",
  reask: "Rewriting the question",
  revise: "Revising the design",
  comments: "Applying comments",
  ask: "Answering",
  artifact: "Making",
  html: "Making HTML",
};

export const busyName = (b: Busy) => BUSY_NAMES[b.kind];

export const getDesigns = (ws: WorkspaceName) => states.get(workspaceKey(ws)) ?? EMPTY;

function set(ws: WorkspaceName, f: (s: DesignsState) => DesignsState) {
  states.set(workspaceKey(ws), f(getDesigns(ws)));
  for (const l of listeners) l();
}

export const setError = (ws: WorkspaceName, error?: string) => set(ws, (s) => ({ ...s, error }));

/** Switches the tab to design `slug`, back on the view its step needs. */
export function selectDesign(ws: WorkspaceName, slug: string) {
  set(ws, (s) =>
    s.current === slug ? s : { ...s, current: slug, view: undefined, doc: undefined },
  );
}

export function setView(ws: WorkspaceName, view: View, doc?: string) {
  set(ws, (s) => ({ ...s, view, doc: doc ?? s.doc }));
}

/** Puts a design the backend returned in place of the old one. */
export function applyDesign(ws: WorkspaceName, d: Design) {
  set(ws, (s) => {
    const has = s.designs.some((x) => x.slug === d.slug);
    const designs = has ? s.designs.map((x) => (x.slug === d.slug ? d : x)) : [...s.designs, d];
    return { ...s, designs, error: undefined };
  });
}

export async function loadDesigns(ws: WorkspaceName) {
  const r = await commands.listDesigns(ws);
  if (r.status === "error") return set(ws, (s) => ({ ...s, loaded: true, error: r.error }));
  set(ws, (s) => {
    const current = r.data.some((d) => d.slug === s.current) ? s.current : r.data[0]?.slug;
    return { ...s, designs: r.data, loaded: true, current };
  });
}

/** Applies a command's design, or shows its error. */
export async function act(ws: WorkspaceName, r: Promise<CommandResult<Design>>) {
  const got = await r;
  if (got.status === "error") setError(ws, got.error);
  else applyDesign(ws, got.data);
  return got;
}

interface RunOptions {
  kind: BusyKind;
  about?: string;
  /** What the Agent runs list says it's about. */
  subject: string;
  /** The text channel streams into `streams` rather than activity. */
  streams?: boolean;
  open?(): void;
}

/** Runs an agent step for design `slug`, showing it as busy and in Agent runs. */
export async function runStep<T>(
  ws: WorkspaceName,
  slug: string,
  o: RunOptions,
  go: (channel: Channel<string>) => Promise<CommandResult<T>>,
  design: (data: T) => Design = (data) => data as Design,
): Promise<CommandResult<T>> {
  const key = `${slug}:${o.about ?? ""}`;
  set(ws, (s) => ({
    ...s,
    error: undefined,
    busy: { ...s.busy, [slug]: { kind: o.kind, about: o.about, activity: "" } },
    streams: o.streams ? { ...s.streams, [key]: "" } : s.streams,
  }));
  const handle = startRun(ws, {
    kind: "design",
    subject: o.subject,
    stop: () => commands.stopDesign(ws, slug),
    retry: () => runStep(ws, slug, o, go, design),
    open: o.open,
  });
  const channel = new Channel<string>();
  channel.onmessage = (text) => {
    if (o.streams)
      set(ws, (s) => ({ ...s, streams: { ...s.streams, [key]: (s.streams[key] ?? "") + text } }));
    else {
      handle.activity(text);
      set(ws, (s) => {
        const b = s.busy[slug];
        return b ? { ...s, busy: { ...s.busy, [slug]: { ...b, activity: text } } } : s;
      });
    }
  };
  const r = await go(channel);
  endFrom(handle, r);
  set(ws, (s) => {
    const { [slug]: _, ...busy } = s.busy;
    const { [key]: __, ...streams } = s.streams;
    return { ...s, busy, streams };
  });
  if (r.status === "error") setError(ws, r.error);
  else applyDesign(ws, design(r.data));
  return r;
}

function watch(ws: WorkspaceName) {
  const key = workspaceKey(ws);
  const w = watchers.get(key);
  if (w) {
    w.count++;
    return;
  }
  const win = getCurrentWebviewWindow();
  const reload = () => loadDesigns(ws);
  const subs = [
    events.repoChanged(win).listen(reload),
    events.designChanged(win).listen((e) => {
      if (workspaceKey(e.payload.workspace) === key) reload();
    }),
  ];
  watchers.set(key, {
    count: 1,
    stop: () => {
      for (const s of subs) s.then((un) => un());
    },
  });
  reload();
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

/** The workspace's designs, kept in step with its files. */
export function useDesigns(ws: WorkspaceName): DesignsState {
  useEffect(() => {
    watch(ws);
    return () => unwatch(ws);
  }, [ws]);
  return useSyncExternalStore(subscribe, () => getDesigns(ws));
}

/** Opens the workspace's Design tab, or brings it forward, on design `slug` and `view`. */
export function showDesign(ws: WorkspaceName, slug?: string, view?: View, doc?: string) {
  if (slug) selectDesign(ws, slug);
  if (view) setView(ws, view, doc);
  const api = dockOf(ws);
  if (api) openDesign(api);
}
