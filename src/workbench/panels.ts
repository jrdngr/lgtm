import type { AddPanelOptions, DockviewApi } from "dockview-react";

type PanelKind = "review" | "terminal" | "report";

const TITLES: Record<PanelKind, string> = {
  review: "Review",
  terminal: "Terminal",
  report: "Reviewers",
};

function addPanel(
  api: DockviewApi,
  component: PanelKind,
  params: Record<string, unknown> = {},
  position?: AddPanelOptions["position"],
) {
  return api.addPanel({
    id: `${component}-${crypto.randomUUID()}`,
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

/** Shows a persona review's reports, in the pane it's already open in if there is one. */
export function openReport(api: DockviewApi, run: number) {
  const open = api.getPanel(`report-${run}`);
  if (open) return open.api.setActive();
  // Beside the review, so jumping to a finding's code keeps the report in view.
  const review = api.panels.find((p) => p.view.contentComponent === "review");
  const position = review ? { referencePanel: review, direction: "right" as const } : undefined;
  api.addPanel({
    id: `report-${run}`,
    component: "report",
    title: "Reviewers",
    params: { run },
    position,
  });
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
