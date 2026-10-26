import { useSyncExternalStore } from "react";
import type { ProjectDto, WorkspaceDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import type { Role } from "../terminal/registry";
import type { CtxEntry } from "../ui/contextMenu";
import type { Activity, AgentInfo } from "../workspaces/activity";
import type { PinColor } from "../workspaces/pins";
import { frameRepo } from "./boot";

/** An agent as another repo's frame lists it. */
export interface TreeAgent extends AgentInfo {
  name: string;
  role?: Role;
}

/** A workspace row as another repo's frame draws it. */
export interface TreeRow {
  w: WorkspaceDto;
  /** Set for a PR workspace outside a stack, which shows as its PR. */
  pr: { number: number; title: string } | null;
  a: { state: Activity["state"]; agents: TreeAgent[] };
  color?: PinColor;
}

/** A repo's workspace rows, pinned ones first. */
export interface Subtree {
  top: TreeRow[];
  rest: TreeRow[];
}

/** What a repo's frame does for the others. */
export interface FrameApi {
  select(ws: WorkspaceName): void;
  /** The workspace row's menu. */
  menu(ws: WorkspaceName): CtxEntry[];
  openAgent(ws: WorkspaceName, agent: AgentInfo): void;
  /** Shows the repo's Delete for the workspace, asking first. */
  askDelete(ws: string): void;
  /** Whether everything running in the repo may go. `before` runs before anything is asked. */
  confirmAll(before: () => void): Promise<boolean>;
  addRepo(): void;
}

/** The project window's own state for its repo list, by repo label. */
export interface ProjectUi {
  pinned: string[];
  colors: Record<string, PinColor>;
  collapsed: string[];
  /** The repo on screen. */
  active?: string;
}

export interface ShellState {
  project: ProjectDto;
  ui: ProjectUi;
  /** The label of the repo on screen. */
  active: string;
  trees: Record<string, Subtree>;
}

/** The project window, as its frames reach it. */
export interface ShellApi {
  state(): ShellState;
  subscribe(l: () => void): () => void;
  publish(label: string, tree: Subtree): void;
  register(label: string, api: FrameApi): () => void;
  /** Shows the repo with `label`, waking it, and selects `ws` in it when given. */
  show(label: string, ws?: WorkspaceName): void;
  /** The repo's frame, once it's up. */
  frame(label: string): FrameApi | undefined;
  togglePin(label: string): void;
  setColor(label: string, color: PinColor | null): void;
  toggleCollapsed(label: string): void;
  /** These resolve to the error, when there is one. */
  sleep(label: string): Promise<string | undefined>;
  wake(label: string): Promise<string | undefined>;
  remove(label: string): Promise<string | undefined>;
  rename(name: string): Promise<string | undefined>;
  /** Reloads the project, after a repo was added. */
  refresh(): Promise<void>;
}

declare global {
  interface Window {
    lgtmShell?: ShellApi;
    /** Unmounts the frame's app, before the frame goes. */
    lgtmTeardown?: () => void;
  }
}

/** The project window this frame is in. */
export const shell = (): ShellApi | undefined =>
  frameRepo ? (window.parent as Window).lgtmShell : undefined;

const none = () => () => {};

/** The project window's state, kept current. Undefined outside a frame. */
export function useShell(): ShellState | undefined {
  const s = shell();
  return useSyncExternalStore(s?.subscribe ?? none, () => s?.state());
}
