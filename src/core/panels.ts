import type {
  AddPanelOptions,
  DockviewApi,
  DockviewGroupPanel,
  IDockviewPanel,
} from "dockview-react";

/** The kinds of tab. A Claude tab is a terminal that starts Claude Code. */
export type PanelKind =
  | "review"
  | "terminal"
  | "claude"
  | "report"
  | "architecture"
  | "guided"
  | "qa"
  | "drawing"
  | "note";

const TITLES: Record<PanelKind, string> = {
  review: "Review",
  terminal: "Terminal",
  claude: "Claude",
  report: "Report",
  architecture: "Architecture",
  guided: "Guided Review",
  qa: "Q&A",
  drawing: "Drawing",
  note: "Note",
};

/** What the new tab menu offers, in order. */
export const KINDS: PanelKind[] = ["claude", "terminal", "review", "guided", "architecture", "qa"];

export const isReviewPanel = (p: IDockviewPanel) => p.view.contentComponent === "review";

export const kindTitle = (kind: PanelKind) => TITLES[kind];

/** Makes `panel` and its pane active. Dockview re-attaches the content of a panel its pane
 * already shows when that panel is set active, which resets the content's scroll, so for a
 * shown panel only its pane is made active. */
export function activate(panel: IDockviewPanel) {
  if (panel.group.activePanel === panel) panel.group.api.setActive();
  else panel.api.setActive();
}

/** Makes `panel` active and puts keyboard focus in it. */
export function focusPanel(panel: IDockviewPanel | undefined) {
  if (!panel) return;
  activate(panel);
  panel.view.content.element
    .querySelector<HTMLElement>("textarea, input, [contenteditable='true'], [tabindex]")
    ?.focus();
}

function addPanel(
  api: DockviewApi,
  component: PanelKind,
  params: Record<string, unknown> = {},
  position?: AddPanelOptions["position"],
  id = `${component}-${crypto.randomUUID()}`,
  inactive = false,
) {
  const terminal = component === "terminal" || component === "claude";
  const panel = api.addPanel({
    id,
    component: component === "claude" ? "terminal" : component,
    title: TITLES[component],
    params: component === "claude" ? { ...params, claude: true } : params,
    position,
    inactive,
    // Terminals keep their DOM while their tab is hidden so scrollback and state survive.
    renderer: terminal ? "always" : "onlyWhenVisible",
  });
  if (inactive) reshowActive(api);
  return panel;
}

/** Puts back the shown tab's content that dockview takes out of its pane when it adds a tab
 * that's always rendered behind it, as restoring a layout does. */
export function reshowActive(api: DockviewApi) {
  for (const g of api.groups) {
    const p = g.activePanel;
    if (p?.api.renderer === "onlyWhenVisible" && !p.view.content.element.isConnected)
      g.model.openPanel(p);
  }
}

/** Opens a terminal in the focused pane, or split beside it when `direction` is given. With `setup`, the
 * shell runs the workspace's `.lgtm/setup` first. */
export function addTerminal(api: DockviewApi, direction?: "right" | "below", setup = false) {
  const active = api.activePanel;
  const position = direction && active ? { referencePanel: active, direction } : inActivePane(api);
  return addPanel(api, "terminal", setup ? { setup } : {}, position);
}

/** The focused pane, where every new tab opens. Panes split only when the user splits them. */
export function inActivePane(api: DockviewApi): AddPanelOptions["position"] {
  return api.activeGroup ? { referenceGroup: api.activeGroup } : undefined;
}

/** Opens a Claude tab titled `title` in the focused pane, behind the tab showing there when
 * there is one. */
export function addClaudeBehind(api: DockviewApi, title: string) {
  const behind = api.panels.length > 0;
  const panel = addPanel(api, "claude", { renamed: true }, inActivePane(api), undefined, behind);
  panel.api.setTitle(title);
  return panel;
}

/** Shows a report, in the pane it's already open in if there is one. */
export function openReport(api: DockviewApi, run: number) {
  const open = api.getPanel(`report-${run}`);
  if (open) return activate(open);
  addPanel(api, "report", { run }, inActivePane(api), `report-${run}`);
}

/** Shows drawing `id`, in the pane it's already open in if there is one. */
export function openDrawing(api: DockviewApi, id: string, name: string) {
  const open = api.getPanel(`drawing-${id}`);
  if (open) return activate(open);
  addPanel(api, "drawing", { drawing: id }, inActivePane(api), `drawing-${id}`).api.setTitle(name);
}

/** Shows note `id`, in the pane it's already open in if there is one. Opens it in the editor
 * with `edit`, else as it last was. */
export function openNote(api: DockviewApi, id: string, name: string, edit = false) {
  const open = api.getPanel(`note-${id}`);
  if (open) return activate(open);
  const params = edit ? { note: id, view: "editor" } : { note: id };
  addPanel(api, "note", params, inActivePane(api), `note-${id}`).api.setTitle(name);
}

/** Shows the Architecture tab, in its pane if it's already open. */
export function openArchitecture(api: DockviewApi) {
  const open = api.getPanel("architecture");
  if (open) return activate(open);
  addPanel(api, "architecture", {}, inActivePane(api), "architecture");
}

/** Shows the Guided Review tab, in its pane if it's already open. */
export function openGuidedTab(api: DockviewApi) {
  const open = api.getPanel("guided");
  if (open) return activate(open);
  addPanel(api, "guided", {}, inActivePane(api), "guided");
}

/** Shows the Q&A tab, or brings it forward. */
export function openQa(api: DockviewApi) {
  const open = api.getPanel("qa");
  if (open) return activate(open);
  addPanel(api, "qa", {}, inActivePane(api), "qa");
}

/** Tabs a dock has one of. */
const SINGLE: PanelKind[] = ["architecture", "guided", "qa"];

/** Opens a `kind` tab in `group`. Architecture, Guided Review and Q&A are one per dock, so an
 * open one moves here. */
export function addToGroup(api: DockviewApi, group: DockviewGroupPanel, kind: PanelKind) {
  const single = SINGLE.includes(kind) ? api.getPanel(kind) : undefined;
  if (single) {
    single.api.moveTo({ group });
    return activate(single);
  }
  addPanel(api, kind, {}, { referenceGroup: group }, SINGLE.includes(kind) ? kind : undefined);
}

/** Splits `group` with an empty pane, which offers the kinds of tab to open in it. */
export function splitGroup(
  api: DockviewApi,
  group: DockviewGroupPanel,
  direction: "right" | "below",
) {
  api.addGroup({ referenceGroup: group, direction });
}

/** Opens a review tab on the workspace's revset in the focused pane. */
export function openReviewTab(api: DockviewApi) {
  return addPanel(api, "review", {}, inActivePane(api));
}

/** A new workspace's main dock: a review. Its terminal opens in the bottom dock. */
export function defaultLayout(api: DockviewApi) {
  addPanel(api, "review");
}

/** Drops panels earlier builds kept in the grid (Tasks, Files, Design, Threads, Graph, Agent
 * runs) and panes that belonged to other workspaces. A System tab becomes the Architecture
 * tab. */
export function dropStalePanels(api: DockviewApi) {
  for (const p of [...api.panels]) {
    if (p.view.contentComponent === "system") {
      if (!api.getPanel("architecture")) {
        addPanel(api, "architecture", {}, { referencePanel: p.id }, "architecture");
      }
      p.api.close();
      continue;
    }
    const task = (p.params as { task?: unknown } | undefined)?.task;
    const moved = ["tasks", "files", "design", "threads", "graph", "runs"].includes(
      p.view.contentComponent,
    );
    if (moved || typeof task === "string") p.api.close();
  }
}
