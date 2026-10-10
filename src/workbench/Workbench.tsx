import {
  type AddPanelOptions,
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  type IDockviewPanelProps,
  themeDark,
} from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import { useEffect, useState } from "react";
import { commands, type RepoInfo, unwrap } from "../ipc";
import { LargeDiffSpike } from "../spikes/LargeDiffSpike";
import { PtyThroughputSpike } from "../spikes/PtyThroughputSpike";
import { TerminalPanel } from "../terminal/TerminalPanel";
import { chord } from "./keys";

function Placeholder({ api }: IDockviewPanelProps) {
  return <div className="placeholder">{api.title}</div>;
}

const components = {
  review: Placeholder,
  files: Placeholder,
  terminal: TerminalPanel,
  largeDiffSpike: LargeDiffSpike,
  ptyThroughputSpike: PtyThroughputSpike,
};

type PanelKind = keyof typeof components;

function addPanel(
  api: DockviewApi,
  component: PanelKind,
  title: string,
  position?: AddPanelOptions["position"],
) {
  api.addPanel({
    id: `${component}-${crypto.randomUUID()}`,
    component,
    title,
    position,
    // Terminals keep their DOM while their tab is hidden so scrollback and state survive.
    renderer: component === "terminal" ? "always" : "onlyWhenVisible",
  });
}

function addTerminal(api: DockviewApi, direction?: "right" | "below") {
  const active = api.activePanel;
  const position = direction && active ? { referencePanel: active, direction } : undefined;
  addPanel(api, "terminal", "Terminal", position);
}

function defaultLayout(api: DockviewApi) {
  addPanel(api, "files", "Files");
  addPanel(api, "review", "Review", { direction: "right" });
  addTerminal(api, "below");
}

const shortcuts: Record<string, (api: DockviewApi) => void> = {
  "Mod+T": (api) => addTerminal(api),
  "Mod+D": (api) => addTerminal(api, "right"),
  "Mod+Shift+D": (api) => addTerminal(api, "below"),
};

async function restoreLayout(api: DockviewApi) {
  const saved = await unwrap(commands.loadLayout()).catch(() => null);
  if (saved) {
    try {
      api.fromJSON(JSON.parse(saved));
      return;
    } catch (e) {
      console.warn("discarding unreadable layout", e);
      api.clear();
    }
  }
  defaultLayout(api);
}

export function Workbench({ repo }: { repo: RepoInfo }) {
  const [api, setApi] = useState<DockviewApi>();

  async function onReady({ api }: DockviewReadyEvent) {
    await restoreLayout(api);
    let timer: number | undefined;
    api.onDidLayoutChange(() => {
      clearTimeout(timer);
      timer = window.setTimeout(() => commands.saveLayout(JSON.stringify(api.toJSON())), 500);
    });
    setApi(api);
  }

  useEffect(() => {
    if (!api) return;
    // Capture phase, so app shortcuts win over whatever panel has focus, including xterm.
    const onKey = (e: KeyboardEvent) => {
      const action = shortcuts[chord(e) ?? ""];
      if (!action) return;
      e.preventDefault();
      e.stopPropagation();
      action(api);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [api]);

  return (
    <div className="workbench">
      <header className="toolbar">
        <span className="repo" title={repo.root}>
          {repo.name}
        </span>
        <button type="button" disabled={!api} onClick={() => api && addTerminal(api)}>
          New terminal
        </button>
        <span className="spacer" />
        <button
          type="button"
          disabled={!api}
          onClick={() => api && addPanel(api, "largeDiffSpike", "Spike: large diff")}
        >
          Spike: large diff
        </button>
        <button
          type="button"
          disabled={!api}
          onClick={() => api && addPanel(api, "ptyThroughputSpike", "Spike: PTY throughput")}
        >
          Spike: PTY throughput
        </button>
      </header>
      <DockviewReact components={components} onReady={onReady} theme={themeDark} />
    </div>
  );
}
