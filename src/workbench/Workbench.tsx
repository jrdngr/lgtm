import type { DockviewApi } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import type { RepoInfo } from "../ipc";
import { FilesPanel } from "../review/FilesPanel";
import { WorkspaceContext, type WorkspaceName, workspaceKey } from "../workspaces/context";
import { WorkspacesSidebar } from "../workspaces/WorkspacesSidebar";
import { chord } from "./keys";
import { addTerminal } from "./panels";
import { SidePanel } from "./SidePanel";
import { SideToggle } from "./SideToggle";
import { loadSides, type Sides, saveSides } from "./sides";
import { WorkspaceDock } from "./WorkspaceDock";

/** Shortcuts acting on the selected workspace's panes. */
const shortcuts: Record<string, (api: DockviewApi) => void> = {
  "Mod+T": (api) => addTerminal(api),
  "Mod+D": (api) => addTerminal(api, "right"),
  "Mod+Shift+D": (api) => addTerminal(api, "below"),
  // Pane to pane, as in iTerm.
  "Mod+]": (api) => api.moveToNext({ includePanel: true }),
  "Mod+[": (api) => api.moveToPrevious({ includePanel: true }),
};

const storageKey = (repo: RepoInfo) => `lgtm:workspace:${repo.root}`;

function savedWorkspace(repo: RepoInfo): WorkspaceName {
  try {
    return localStorage.getItem(storageKey(repo)) || null;
  } catch {
    return null;
  }
}

interface Mounted {
  name: WorkspaceName;
  setup: boolean;
}

/** Side panels around the selected workspace's dock. Each workspace has its own dock
 * and layout. Docks of other workspaces stay mounted but hidden, so their terminals run on. */
export function Workbench({ repo }: { repo: RepoInfo }) {
  const [selected, setSelected] = useState<WorkspaceName>(() => savedWorkspace(repo));
  const [mounted, setMounted] = useState<Mounted[]>([{ name: selected, setup: false }]);
  const [sides, setSidesState] = useState<Sides>(() => loadSides(repo.root));
  const leftBody = useRef<HTMLDivElement>(null);
  const rightBody = useRef<HTMLDivElement>(null);
  const apis = useRef(new Map<string, DockviewApi>());
  const api = () => apis.current.get(workspaceKey(selected));

  function select(name: WorkspaceName, setup = false) {
    setSelected(name);
    setMounted((m) => (m.some((d) => d.name === name) ? m : [...m, { name, setup }]));
    try {
      localStorage.setItem(storageKey(repo), name ?? "");
    } catch {
      // Remembering the workspace is a convenience.
    }
  }

  /** Unmounts a deleted workspace's dock, which ends its terminals. */
  function forget(name: string) {
    if (selected === name) select(null);
    apis.current.delete(name);
    setMounted((m) => m.filter((d) => d.name !== name));
  }

  function setSides(update: (s: Sides) => Sides) {
    setSidesState((s) => {
      const next = update(s);
      saveSides(repo.root, next);
      return next;
    });
  }

  const toggle = (side: keyof Sides) =>
    setSides((s) => ({ ...s, [side]: { ...s[side], open: !s[side].open } }));

  /** Opens the side panel on `view` and focuses it. */
  function showView(side: keyof Sides, view: string) {
    setSides((s) => ({ ...s, [side]: { ...s[side], open: true, view } }));
    const body = side === "left" ? leftBody : rightBody;
    requestAnimationFrame(() => body.current?.focus());
  }

  const panelKeys: Record<string, () => void> = {
    "Mod+B": () => toggle("left"),
    "Mod+Alt+B": () => toggle("right"),
    "Mod+Shift+K": () => showView("left", "workspaces"),
    "Mod+Shift+E": () => showView("right", "files"),
  };

  const apiRef = useRef(api);
  apiRef.current = api;
  const panelKeysRef = useRef(panelKeys);
  panelKeysRef.current = panelKeys;

  useEffect(() => {
    // Capture phase, so app shortcuts win over whatever panel has focus, including xterm.
    const onKey = (e: KeyboardEvent) => {
      const key = chord(e) ?? "";
      const panelKey = panelKeysRef.current[key];
      if (panelKey) {
        e.preventDefault();
        e.stopPropagation();
        panelKey();
        return;
      }
      const action = shortcuts[key];
      const target = apiRef.current();
      if (!action || !target) return;
      e.preventDefault();
      e.stopPropagation();
      action(target);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  const withApi = (f: (api: DockviewApi) => void) => () => {
    const a = api();
    if (a) f(a);
  };

  return (
    <div className="workbench">
      <header className="toolbar">
        <SideToggle
          side="left"
          open={sides.left.open}
          shortcut="⌘B"
          onClick={() => toggle("left")}
        />
        <span className="repo" title={repo.root}>
          {repo.name}
        </span>
        <button type="button" onClick={withApi((a) => addTerminal(a))}>
          New terminal
        </button>
        <span className="spacer" />
        <SideToggle
          side="right"
          open={sides.right.open}
          shortcut="⌥⌘B"
          onClick={() => toggle("right")}
        />
      </header>
      <div className="workbench-body">
        <SidePanel
          side="left"
          state={sides.left}
          onChange={(left) => setSides((s) => ({ ...s, left }))}
          bodyRef={leftBody}
          views={[
            {
              id: "workspaces",
              title: "Workspaces",
              render: () => (
                <WorkspacesSidebar
                  selected={selected}
                  onSelect={select}
                  onDeleted={forget}
                  onMissing={() => select(null)}
                />
              ),
            },
          ]}
        />
        <div className="docks">
          {mounted.map((d) => (
            <WorkspaceDock
              key={workspaceKey(d.name)}
              workspace={d.name}
              hidden={d.name !== selected}
              setup={d.setup}
              onReady={(a) => apis.current.set(workspaceKey(d.name), a)}
            />
          ))}
        </div>
        <WorkspaceContext.Provider value={selected}>
          <SidePanel
            side="right"
            state={sides.right}
            onChange={(right) => setSides((s) => ({ ...s, right }))}
            bodyRef={rightBody}
            views={[{ id: "files", title: "Files", render: () => <FilesPanel /> }]}
          />
        </WorkspaceContext.Provider>
      </div>
    </div>
  );
}
