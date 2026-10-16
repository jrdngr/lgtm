import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useRef } from "react";
import { flushSync } from "react-dom";
import { commands } from "../ipc";
import { openMenu } from "../ui/contextMenu";
import { bottom } from "../workbench/bottom";
import { useWorkspace } from "../workspaces/context";
import { watchAgentHooks } from "./agentHooks";
import { registerTerminal, touchTerminal } from "./registry";
import { resumeCommand } from "./resume";
import { openSession, releaseSession } from "./sessions";
import { shellPaths } from "./shellPaths";
import { terminalMenu } from "./terminalMenu";
import { isAgentTitle, titleLabel } from "./title";

type Params = {
  setup?: boolean;
  /** The Claude Code session running in it, which a restart resumes. */
  claudeSession?: string;
  /** The user named the tab, so its agent's title doesn't. */
  renamed?: boolean;
};

export function TerminalPanel({ api, containerApi, params }: IDockviewPanelProps<Params>) {
  const ref = useRef<HTMLDivElement>(null);
  const workspace = useWorkspace();
  const setup = useRef(params.setup ?? false);

  useEffect(() => {
    const container = ref.current as HTMLDivElement;
    const resume = resumeCommand(api.getParameters<Params>().claudeSession);
    const session = openSession(api.id, container, workspace, setup.current, resume);
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
      submit: session.submit,
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
    // Dropped files paste their paths, from Finder or from the app's file trees.
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
        term.paste(shellPaths(paths));
        api.setActive();
        term.focus();
      });
    };
    container.addEventListener("dragover", onDragOver);
    container.addEventListener("drop", onDrop);

    // The agent's session is kept for a restart, and its title names the tab.
    const unwatch = watchAgentHooks((r) => {
      if (r.terminal !== api.id) return;
      const saved = api.getParameters<Params>().claudeSession;
      if (r.ended) api.updateParameters({ claudeSession: undefined });
      else if (r.session && r.session !== saved) api.updateParameters({ claudeSession: r.session });
    });
    const titled = term.onTitleChange((t) => {
      if (isAgentTitle(t) && !api.getParameters<Params>().renamed) api.setTitle(titleLabel(t));
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
