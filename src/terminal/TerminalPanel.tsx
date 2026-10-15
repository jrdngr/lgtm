import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useRef } from "react";
import { flushSync } from "react-dom";
import { openMenu } from "../ui/contextMenu";
import { bottom } from "../workbench/bottom";
import { useWorkspace } from "../workspaces/context";
import { registerTerminal, touchTerminal } from "./registry";
import { openSession, releaseSession } from "./sessions";
import { terminalMenu } from "./terminalMenu";

type Params = { setup?: boolean };

export function TerminalPanel({ api, containerApi, params }: IDockviewPanelProps<Params>) {
  const ref = useRef<HTMLDivElement>(null);
  const workspace = useWorkspace();
  const setup = useRef(params.setup ?? false);

  useEffect(() => {
    const container = ref.current as HTMLDivElement;
    const session = openSession(api.id, container, workspace, setup.current);
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

    const sub = api.onDidActiveChange(({ isActive }) => isActive && term.focus());
    const observer = new ResizeObserver(session.refit);
    observer.observe(container);
    if (api.isActive) term.focus();

    return () => {
      unregister();
      term.textarea?.removeEventListener("focus", onFocus);
      container.removeEventListener("contextmenu", onMenu);
      observer.disconnect();
      sub.dispose();
      releaseSession(api.id);
    };
  }, [api, containerApi, workspace]);

  return <div ref={ref} className="terminal" />;
}
