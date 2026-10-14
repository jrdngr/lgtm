import type { DockviewApi } from "dockview-react";
import { inActivePane } from "../workbench/panels";

/** The one preview tab per dock, which each selected PR replaces. */
const ID = "pr-preview";

const urlOf = (api: DockviewApi) => (api.getPanel(ID)?.params as { url?: string } | undefined)?.url;

/** Shows the PR in the dock's preview tab, opening it in the active pane if needed. */
export function openPrPreview(api: DockviewApi, pr: { number: number; url: string }) {
  const title = `PR #${pr.number}`;
  const open = api.getPanel(ID);
  if (!open) {
    api.addPanel({
      id: ID,
      component: "prPreview",
      title,
      params: { url: pr.url },
      position: inActivePane(api),
    });
    return;
  }
  if (urlOf(api) !== pr.url) open.api.updateParameters({ url: pr.url });
  open.api.setTitle(title);
  open.api.setActive();
}

/** Closes the preview tab if it shows `url`, once the PR has a workspace of its own. */
export function closePrPreview(api: DockviewApi, url: string) {
  if (urlOf(api) === url) api.getPanel(ID)?.api.close();
}
