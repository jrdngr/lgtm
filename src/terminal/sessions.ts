import { Channel } from "@tauri-apps/api/core";
import type { Terminal } from "@xterm/xterm";
import { commands, type PtyEvent } from "../ipc";
import { type ActivityState, pulse, report } from "../workspaces/activity";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";
import { onAgentHook } from "./agentHooks";
import { isAgentTitle, isBusyTitle, titleLabel } from "./title";
import { createTerminal } from "./xterm";

/** An xterm and the shell it runs, in an element its panel shows. */
export interface Session {
  host: HTMLDivElement;
  term: Terminal;
  refit(): void;
  /** The title its program set, if any. */
  title(): string;
  /** The program in the foreground, when it isn't the shell. */
  running(): Promise<string | null>;
  dispose(): void;
}

/** How long a session outlives its panels, waiting for a panel with the same id to take it. */
const KEEP_MS = 1000;

/** Sessions by panel id, with their workspace and how many mounted panels show them. */
const sessions = new Map<
  string,
  { session: Session; workspace: string; panels: number; timer?: number }
>();

/** The programs running in the sessions of the panels `pick` takes, by panel id and
 * workspace key. With `named`, each is `workspace: program`. */
export async function runningPrograms(
  pick: (panel: string, workspace: string) => boolean,
  named = false,
): Promise<string[]> {
  const picked = [...sessions].filter(([id, s]) => pick(id, s.workspace));
  const names = await Promise.all(
    picked.map(async ([, s]) => {
      const program = await s.session.running();
      if (program === null || !named) return program;
      return `${s.workspace || "default"}: ${program}`;
    }),
  );
  return names.filter((n) => n !== null);
}

/** Shows panel `id`'s session in `container`: the one a panel with this id has, when the
 * panel is moving to the other dock or React mounts it again, or else a new shell. */
export function openSession(
  id: string,
  container: HTMLElement,
  workspace: WorkspaceName,
  setup: boolean,
): Session {
  const open = sessions.get(id);
  if (open) {
    clearTimeout(open.timer);
    open.panels++;
    container.append(open.session.host);
    open.session.refit();
    return open.session;
  }
  const host = document.createElement("div");
  host.className = "terminal-host";
  container.append(host);
  const session = startSession(id, host, workspace, setup);
  sessions.set(id, { session, workspace: workspaceKey(workspace), panels: 1 });
  return session;
}

/** Ends panel `id`'s session once no panel shows it, unless one opens it again soon. */
export function releaseSession(id: string) {
  const open = sessions.get(id);
  if (!open || --open.panels > 0) return;
  open.timer = window.setTimeout(() => {
    sessions.delete(id);
    open.session.dispose();
  }, KEEP_MS);
}

function startSession(
  id: string,
  host: HTMLDivElement,
  workspace: WorkspaceName,
  setup: boolean,
): Session {
  const { term, refit, unwatch } = createTerminal(host);
  let pty: number | undefined;
  let disposed = false;
  let title = "";
  const activity = `term:${id}`;
  /** How its agent stands, as last reported. */
  let agent: ActivityState = "gone";
  /** Whether Claude Code's hooks report for it, which then wins over the title. */
  let hooked = false;
  const set = (state: ActivityState) => {
    agent = state;
    report(workspace, activity, state, titleLabel(title), true);
  };

  const channel = new Channel<PtyEvent>();
  channel.onmessage = (e) => {
    if (e.kind === "data") {
      term.write(e.data);
      pulse(activity);
    } else {
      term.write(`\r\n[process exited with code ${e.code}]\r\n`);
      const busy = agent === "running" || agent === "input";
      set(busy && e.code !== 0 ? "failed" : "gone");
    }
  };
  const unhook = onAgentHook(id, (state) => {
    hooked = true;
    set(state === "working" ? "running" : state);
    pulse(activity);
  });

  const { cols, rows } = term;
  commands.ptySpawn(workspace, id, cols, rows, setup, channel).then((r) => {
    if (r.status === "error") {
      term.write(`failed to start shell: ${r.error}\r\n`);
    } else if (disposed) {
      commands.ptyKill(r.data);
    } else {
      pty = r.data;
      // Resizes while it started weren't sent.
      if (term.cols !== cols || term.rows !== rows) commands.ptyResize(pty, term.cols, term.rows);
    }
  });

  const subs = [
    term.onData((data) => pty !== undefined && commands.ptyWrite(pty, data)),
    term.onTitleChange((t) => {
      title = t;
      if (hooked) set(agent);
      else if (isBusyTitle(t)) set("running");
      else if (isAgentTitle(t)) set(agent === "running" ? "done" : "idle");
      else set("gone");
    }),
    term.onResize(({ cols, rows }) => pty !== undefined && commands.ptyResize(pty, cols, rows)),
  ];

  return {
    host,
    term,
    refit,
    title: () => title,
    running: async () => (pty === undefined ? null : commands.ptyRunning(pty)),
    dispose() {
      disposed = true;
      set("gone");
      unhook();
      for (const s of subs) s.dispose();
      if (pty !== undefined) commands.ptyKill(pty);
      unwatch();
      term.dispose();
      host.remove();
    },
  };
}
