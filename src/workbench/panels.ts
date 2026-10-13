import type {
  AddPanelOptions,
  DockviewApi,
  DockviewGroupPanel,
  IDockviewPanel,
} from "dockview-react";

export type PanelKind = "review" | "terminal" | "report" | "graph" | "threads" | "runs" | "qa";

const TITLES: Record<PanelKind, string> = {
  review: "Review",
  terminal: "Terminal",
  report: "Reviewers",
  graph: "Graph",
  threads: "Threads",
  runs: "Agent runs",
  qa: "Q&A",
};

/** What the new tab menu offers, in order. */
export const KINDS: PanelKind[] = ["review", "terminal", "graph", "qa", "runs"];

export const isReviewPanel = (p: IDockviewPanel) => p.view.contentComponent === "review";

export const kindTitle = (kind: PanelKind) => TITLES[kind];

/** Makes `panel` active and puts keyboard focus in it. */
export function focusPanel(panel: IDockviewPanel | undefined) {
  if (!panel) return;
  panel.api.setActive();
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
) {
  return api.addPanel({
    id,
    component,
    title: TITLES[component],
    params,
    position,
    // Terminals keep their DOM while their tab is hidden so scrollback and state survive.
    renderer: component === "terminal" ? "always" : "onlyWhenVisible",
  });
}

/** Opens a terminal, beside the active panel when `direction` is given. With `setup`, the
 * shell runs the workspace's `.lgtm/setup` first. */
export function addTerminal(api: DockviewApi, direction?: "right" | "below", setup = false) {
  const active = api.activePanel;
  const position = direction && active ? { referencePanel: active, direction } : undefined;
  return addPanel(api, "terminal", setup ? { setup } : {}, position);
}

/** Beside the review, so jumping to code from the new pane keeps it in view. */
function besideReview(api: DockviewApi): AddPanelOptions["position"] {
  const review = api.panels.find(isReviewPanel);
  return review ? { referencePanel: review, direction: "right" } : undefined;
}

/** Shows a persona review's reports, in the pane it's already open in if there is one. */
export function openReport(api: DockviewApi, run: number) {
  const open = api.getPanel(`report-${run}`);
  if (open) return open.api.setActive();
  addPanel(api, "report", { run }, besideReview(api), `report-${run}`);
}

/** Shows the symbol graph beside the review, in its pane if it's already open. */
export function openGraph(api: DockviewApi) {
  const open = api.getPanel("graph");
  if (open) return open.api.setActive();
  addPanel(api, "graph", {}, besideReview(api), "graph");
}

/** Shows the Threads tab beside the review, in its pane if it's already open. */
export function openThreads(api: DockviewApi) {
  const open = api.getPanel("threads");
  if (open) return open.api.setActive();
  addPanel(api, "threads", {}, besideReview(api), "threads");
}

/** Shows the Q&A tab as a tab in the review's pane, or brings it forward. */
export function openQa(api: DockviewApi) {
  const open = api.getPanel("qa");
  if (open) return open.api.setActive();
  const review = api.panels.find(isReviewPanel);
  addPanel(api, "qa", {}, review ? { referencePanel: review } : undefined, "qa");
}

/** Tabs a dock has one of. */
const SINGLE: PanelKind[] = ["graph", "qa"];

/** Opens a `kind` tab in `group`. The graph and Q&A are one per dock, so an open one moves
 * here. */
export function addToGroup(api: DockviewApi, group: DockviewGroupPanel, kind: PanelKind) {
  const single = SINGLE.includes(kind) ? api.getPanel(kind) : undefined;
  if (single) {
    single.api.moveTo({ group });
    return single.api.setActive();
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

/** Opens a review pane on the workspace's revset, as a tab in the largest pane. */
export function openReviewTab(api: DockviewApi) {
  const area = (g: DockviewGroupPanel) => g.width * g.height;
  const largest = api.groups.reduce<DockviewGroupPanel | undefined>(
    (best, g) => (!best || area(g) > area(best) ? g : best),
    undefined,
  );
  return addPanel(api, "review", {}, largest ? { referenceGroup: largest } : undefined);
}

/** A review with a terminal below it. */
export function defaultLayout(api: DockviewApi, setup = false) {
  addPanel(api, "review");
  addTerminal(api, "below", setup);
}

/** Drops panels earlier builds kept in the grid (Tasks, Files) and panes that belonged to
 * other workspaces. */
export function dropStalePanels(api: DockviewApi) {
  for (const p of [...api.panels]) {
    const task = (p.params as { task?: unknown } | undefined)?.task;
    const moved = ["tasks", "files"].includes(p.view.contentComponent);
    if (moved || typeof task === "string") p.api.close();
  }
}
