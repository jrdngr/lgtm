import type { ProjectDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import type { FrameApi, ProjectUi, ShellApi, ShellState, Subtree } from "../frame/shell";
import { commands } from "../ipc";
import { COLORS, type PinColor } from "../workspaces/pins";

/** The project window: its project, and each awake repo's frame. */
let state: ShellState | undefined;
const listeners = new Set<() => void>();
const frames = new Map<string, FrameApi>();
/** Workspaces to select in frames still loading. */
const pending = new Map<string, WorkspaceName>();
const iframes = new Map<string, HTMLIFrameElement>();

const EMPTY: ProjectUi = { pinned: [], colors: {}, collapsed: [] };

const strings = (v: unknown): string[] =>
  Array.isArray(v) ? v.filter((s): s is string => typeof s === "string") : [];

/** The saved list state, with anything unreadable dropped. */
export function parseUi(text: string): ProjectUi {
  try {
    const v = JSON.parse(text) as Record<string, unknown> | null;
    if (!v || typeof v !== "object") return EMPTY;
    const colors = Object.entries((v.colors as Record<string, unknown>) ?? {}).filter(
      (e): e is [string, PinColor] => typeof e[1] === "string" && e[1] in COLORS,
    );
    return {
      pinned: strings(v.pinned),
      colors: Object.fromEntries(colors),
      collapsed: strings(v.collapsed),
      active: typeof v.active === "string" ? v.active : undefined,
    };
  } catch {
    return EMPTY;
  }
}

/** The awake repo with label `want`, else the first awake one. Empty when none is awake. */
export function pickActive(p: ProjectDto, want?: string | null): string {
  const awake = p.repos.filter((r) => !r.asleep);
  return (awake.find((r) => r.label === want) ?? awake[0])?.label ?? "";
}

function set(next: ShellState) {
  state = next;
  for (const l of listeners) l();
}

const now = () => state as ShellState;

/** Loads the window's project. False in a window without one, the start window. */
export async function loadShell(): Promise<boolean> {
  const p = await commands.currentProject();
  if (!p) return false;
  const ui = parseUi(p.ui);
  const active = pickActive(p, p.active ?? ui.active);
  set({ project: p, ui, active, trees: {} });
  // A window opens with a repo running.
  const first = p.repos[0];
  if (active) commands.setActiveRepo(active);
  else if (first) await wake(first.label);
  return true;
}

async function refresh() {
  const p = await commands.currentProject();
  if (!p) return;
  const { trees, active } = now();
  const awake = new Set(p.repos.filter((r) => !r.asleep).map((r) => r.label));
  set({
    ...now(),
    project: p,
    active: pickActive(p, active),
    trees: Object.fromEntries(Object.entries(trees).filter(([label]) => awake.has(label))),
  });
}

function updateUi(f: (ui: ProjectUi) => ProjectUi) {
  const ui = f(now().ui);
  set({ ...now(), ui });
  commands.saveProjectUi(JSON.stringify(ui));
}

function setActive(label: string) {
  if (now().active !== label) {
    set({ ...now(), active: label });
    commands.setActiveRepo(label);
  }
  if (now().ui.active !== label) updateUi((ui) => ({ ...ui, active: label }));
  requestAnimationFrame(() => iframes.get(label)?.contentWindow?.focus());
}

const awake = () => now().project.repos.filter((r) => !r.asleep);

function select(label: string, ws: WorkspaceName) {
  const frame = frames.get(label);
  if (frame) frame.select(ws);
  else pending.set(label, ws);
}

async function show(label: string, ws?: WorkspaceName) {
  const repo = now().project.repos.find((r) => r.label === label);
  if (!repo) return;
  if (repo.asleep && (await wake(label))) return;
  setActive(label);
  if (ws !== undefined) select(label, ws);
}

/** Moves off repo `label` to another awake one. False when there's none. */
function leave(label: string): boolean {
  if (now().active !== label) return true;
  const next = awake().find((r) => r.label !== label);
  if (!next) return false;
  setActive(next.label);
  return true;
}

/** Unmounts repo `label`'s frame and drops it. Its listeners stop before the frame goes. */
async function drop(label: string) {
  iframes.get(label)?.contentWindow?.lgtmTeardown?.();
  frames.delete(label);
  pending.delete(label);
  await new Promise((done) => setTimeout(done, 50));
  const { project, trees } = now();
  const { [label]: _, ...rest } = trees;
  set({
    ...now(),
    project: {
      ...project,
      repos: project.repos.map((r) => (r.label === label ? { ...r, asleep: true } : r)),
    },
    trees: rest,
  });
}

/** Whether everything running in repo `label` may go, showing it before anything is asked. */
const mayGo = (label: string) =>
  frames.get(label)?.confirmAll(() => setActive(label)) ?? Promise.resolve(true);

async function sleep(label: string): Promise<string | undefined> {
  if (awake().length <= 1 || !(await mayGo(label)) || !leave(label)) return;
  await drop(label);
  const r = await commands.sleepRepo(label);
  await refresh();
  return r.status === "error" ? r.error : undefined;
}

/** Wakes repo `label` and shows it. The error, when it can't. */
async function wake(label: string): Promise<string | undefined> {
  const r = await commands.wakeRepo(label);
  if (r.status === "error") return r.error;
  await refresh();
  setActive(label);
}

async function remove(label: string): Promise<string | undefined> {
  const repos = now().project.repos;
  const repo = repos.find((r) => r.label === label);
  if (!repo || repos.length <= 1) return;
  if (!repo.asleep) {
    if (!(await mayGo(label))) return;
    if (!leave(label)) {
      const next = repos.find((r) => r.label !== label);
      const failed = next && (await wake(next.label));
      if (failed) return failed;
    }
    await drop(label);
  }
  const r = await commands.removeRepo(label);
  updateUi((ui) => {
    const { [label]: _, ...colors } = ui.colors;
    return {
      ...ui,
      colors,
      pinned: ui.pinned.filter((l) => l !== label),
      collapsed: ui.collapsed.filter((l) => l !== label),
    };
  });
  await refresh();
  return r.status === "error" ? r.error : undefined;
}

const toggled = (list: string[], label: string) =>
  list.includes(label) ? list.filter((l) => l !== label) : [...list, label];

export const api: ShellApi = {
  state: now,
  subscribe(l) {
    listeners.add(l);
    return () => listeners.delete(l);
  },
  publish(label, tree: Subtree) {
    if (!frames.has(label)) return;
    set({ ...now(), trees: { ...now().trees, [label]: tree } });
  },
  register(label, frame) {
    frames.set(label, frame);
    const ws = pending.get(label);
    pending.delete(label);
    if (ws !== undefined) frame.select(ws);
    return () => {
      if (frames.get(label) === frame) frames.delete(label);
    };
  },
  show: (label, ws) => void show(label, ws),
  frame: (label) => frames.get(label),
  togglePin: (label) => updateUi((ui) => ({ ...ui, pinned: toggled(ui.pinned, label) })),
  setColor: (label, color) =>
    updateUi((ui) => {
      const { [label]: _, ...colors } = ui.colors;
      return { ...ui, colors: color ? { ...colors, [label]: color } : colors };
    }),
  toggleCollapsed: (label) =>
    updateUi((ui) => ({ ...ui, collapsed: toggled(ui.collapsed, label) })),
  sleep,
  wake,
  remove,
  async rename(name) {
    const r = await commands.renameProject(name);
    await refresh();
    return r.status === "error" ? r.error : undefined;
  },
  refresh,
};

/** Asks each repo whether what runs in it may go, as closing the window does. */
export async function confirmClose(): Promise<boolean> {
  for (const label of [...frames.keys()]) if (!(await mayGo(label))) return false;
  return true;
}

/** The repo on screen's frame, for the menu's Add Repository. */
export const activeFrame = () => frames.get(now().active);

export function setIframe(label: string, el: HTMLIFrameElement | null) {
  if (el) iframes.set(label, el);
  else iframes.delete(label);
}

/** Shows repo `label` once the project is reloaded, as opening a repo another way asks. */
export async function showRepo(label: string) {
  await refresh();
  await show(label);
}

window.lgtmShell = api;
