import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useRef } from "react";
import { flushSync } from "react-dom";
import { bottom } from "../core/bottom";
import { useWorkspace, workspaceRoot } from "../core/workspace";
import { commands } from "../ipc";
import { repoRelative } from "../qa/readFirstPaths";
import { loadedSettings } from "../settings/store";
import { openMenu } from "../ui/contextMenu";
import { watchAgentHooks } from "./agentHooks";
import { registerTerminal, touchTerminal } from "./registry";
import { resumeCommand } from "./resume";
import { openSession, releaseSession } from "./sessions";
import { shellPaths } from "./shellPaths";
import { terminalMenu } from "./terminalMenu";
import { isAgentTitle, titleLabel } from "./title";

type Params = {
  setup?: boolean;
  /** Starts Claude Code once the shell is up, for a new Claude tab. */
  claude?: boolean;
  /** The Claude Code session running in it, which a restart resumes. */
  claudeSession?: string;
  /** The user named the tab, so its agent's title doesn't. */
  renamed?: boolean;
};

export function TerminalPanel({ api, containerApi, params }: IDockviewPanelProps<Params>) {
  const ref = useRef<HTMLDivElement>(null);
  const workspace = useWorkspace();
  const setup = useRef(params.setup ?? false);
  const initial = useRef(params);

  useEffect(() => {
    const container = ref.current as HTMLDivElement;
    // The panel's parameters as they stand. The api's own copy holds only the last update.
    const current = () => (containerApi.getPanel(api.id)?.params ?? initial.current) as Params;
    const { claude, claudeSession } = current();
    // A new Claude tab starts Claude Code, and a terminal coming back resumes its session.
    const command =
      claude || claudeSession
        ? loadedSettings().then((s) =>
            claude ? s.claudeTerminal : resumeCommand(s.claudeTerminal, claudeSession),
          )
        : undefined;
    const session = openSession(api.id, container, workspace, setup.current, command);
    if (claude) api.updateParameters({ claude: undefined });
    const { term } = session;
    // Setup runs once, not again when the layout is restored.
    if (setup.current) {
      setup.current = false;
      api.updateParameters({ setup: undefined });
    }

    const unregister = registerTerminal(api.id, {
      workspace,
      title: () => api.title || session.title(),
      canPaste: () => term.modes.bracketedPasteMode,
      paste: (text) => term.paste(text),
      hasAgent: session.hasAgent,
      agentBusy: session.agentBusy,
      prompted: session.prompted,
      submit: session.submit,
      interrupt: session.interrupt,
      focus: () => {
        // A terminal in the hidden bottom panel shows it first, or it can't take focus.
        if (container.closest(".bottom-panel")) flushSync(() => bottom.set({ open: true }));
        api.setActive();
        term.focus();
      },
    });
    const onFocus = () => touchTerminal(api.id);
    term.textarea?.addEventListener("focus", onFocus);
    const onMenu = (e: MouseEvent) => {
      const panel = containerApi.getPanel(api.id);
      if (!panel) return;
      const inBottom = !!container.closest(".bottom-panel");
      openMenu(e, terminalMenu({ term, workspace, api: containerApi, panel, inBottom }));
    };
    // Bubble phase, after xterm selects the word under the pointer, so Copy can take it.
    container.addEventListener("contextmenu", onMenu);
    // Dropped files paste their paths, from Finder or from the app's file trees: relative to
    // the workspace's root when inside it.
    const files = (e: DragEvent) => !!e.dataTransfer?.types.includes("Files");
    const onDragOver = (e: DragEvent) => {
      if (!files(e)) return;
      e.preventDefault();
      if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    };
    const onDrop = (e: DragEvent) => {
      if (!files(e)) return;
      e.preventDefault();
      commands.droppedPaths().then((paths) => {
        if (!paths.length) return;
        const root = workspaceRoot(workspace);
        term.paste(shellPaths(paths.map((p) => repoRelative(p, root))));
        api.setActive();
        term.focus();
      });
    };
    container.addEventListener("dragover", onDragOver);
    container.addEventListener("drop", onDrop);

    // The agent's session is kept for a restart, and its title names the tab.
    const unwatch = watchAgentHooks((r) => {
      if (r.terminal !== api.id) return;
      const saved = current().claudeSession;
      if (r.ended) api.updateParameters({ claudeSession: undefined });
      else if (r.session && r.session !== saved) api.updateParameters({ claudeSession: r.session });
    });
    const titled = term.onTitleChange((t) => {
      if (isAgentTitle(t) && !current().renamed) api.setTitle(titleLabel(t));
    });

    const sub = api.onDidActiveChange(({ isActive }) => isActive && term.focus());
    const observer = new ResizeObserver(session.refit);
    observer.observe(container);
    if (api.isActive) term.focus();

    return () => {
      unregister();
      term.textarea?.removeEventListener("focus", onFocus);
      container.removeEventListener("contextmenu", onMenu);
      container.removeEventListener("dragover", onDragOver);
      container.removeEventListener("drop", onDrop);
      observer.disconnect();
      sub.dispose();
      unwatch();
      titled.dispose();
      releaseSession(api.id);
    };
  }, [api, containerApi, workspace]);

  return <div ref={ref} className="terminal" />;
}
