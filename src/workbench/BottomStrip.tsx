import type { DockviewApi } from "dockview-react";
import { useEffect, useReducer, useSyncExternalStore } from "react";
import { flushSync } from "react-dom";
import type { WorkspaceName } from "../workspaces/context";
import { bottom } from "./bottom";
import { bottomDockOf, docksVersion, subscribeDocks } from "./docks";
import { FoldIcon, useSendMark } from "./FoldStrip";
import { activate } from "./panels";

/** The workspace's bottom dock, re-rendering as its tabs come, go or retitle. */
export function useBottomDock(workspace: WorkspaceName): DockviewApi | undefined {
  useSyncExternalStore(subscribeDocks, docksVersion);
  const api = bottomDockOf(workspace);
  const [n, tick] = useReducer((n: number) => n + 1, 0);
  // Resubscribed on every change, so tabs added since report their title changes too.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `n` is what renews the subscriptions
  useEffect(() => {
    if (!api) return;
    const subs = [
      api.onDidAddPanel(tick),
      api.onDidRemovePanel(tick),
      api.onDidLayoutChange(tick),
      ...api.panels.map((p) => p.api.onDidTitleChange(tick)),
    ];
    return () => {
      for (const s of subs) s.dispose();
    };
  }, [api, n]);
  return api;
}

/** The collapsed bottom panel: every tab's title, which opens the panel on that tab. */
export function BottomStrip({ api, workspace }: { api: DockviewApi; workspace: WorkspaceName }) {
  const sendMark = useSendMark(workspace);
  const open = () => flushSync(() => bottom.set({ open: true }));
  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: each title is a button; the rest of the strip is extra room to click
    // biome-ignore lint/a11y/noStaticElementInteractions: as above
    <div
      className="fold-strip y bottom-strip"
      onClick={(e) => e.target === e.currentTarget && open()}
    >
      <button type="button" className="fold-open" title="Expand" aria-label="Expand" onClick={open}>
        <FoldIcon out />
      </button>
      {api.panels.map((p) => (
        <button
          type="button"
          key={p.id}
          className="fold-tab"
          data-send={sendMark(p.id)}
          onClick={() => {
            open();
            activate(p);
          }}
        >
          <span>{p.title}</span>
        </button>
      ))}
    </div>
  );
}
