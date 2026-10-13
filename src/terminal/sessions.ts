import { Channel } from "@tauri-apps/api/core";
import type { Terminal } from "@xterm/xterm";
import { commands, type PtyEvent } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import { createTerminal } from "./xterm";

/** An xterm and the shell it runs, in an element its panel shows. */
export interface Session {
  host: HTMLDivElement;
  term: Terminal;
  refit(): void;
  /** The title its program set, if any. */
  title(): string;
  dispose(): void;
}

/** How long a session outlives its panels, waiting for a panel with the same id to take it. */
const KEEP_MS = 1000;

/** Sessions by panel id, with how many mounted panels show them. */
const sessions = new Map<string, { session: Session; panels: number; timer?: number }>();

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
  const session = startSession(host, workspace, setup);
  sessions.set(id, { session, panels: 1 });
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

function startSession(host: HTMLDivElement, workspace: WorkspaceName, setup: boolean): Session {
  const { term, refit, unwatch } = createTerminal(host);
  let pty: number | undefined;
  let disposed = false;
  let title = "";

  const channel = new Channel<PtyEvent>();
  channel.onmessage = (e) => {
    if (e.kind === "data") term.write(e.data);
    else term.write(`\r\n[process exited with code ${e.code}]\r\n`);
  };

  const { cols, rows } = term;
  commands.ptySpawn(workspace, cols, rows, setup, channel).then((r) => {
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
    }),
    term.onResize(({ cols, rows }) => pty !== undefined && commands.ptyResize(pty, cols, rows)),
  ];

  return {
    host,
    term,
    refit,
    title: () => title,
    dispose() {
      disposed = true;
      for (const s of subs) s.dispose();
      if (pty !== undefined) commands.ptyKill(pty);
      unwatch();
      term.dispose();
      host.remove();
    },
  };
}
