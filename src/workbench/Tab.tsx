import { DockviewDefaultTab, type IDockviewPanelHeaderProps } from "dockview-react";
import { confirmClose } from "../editor/unsaved";
import { openMenu } from "../ui/contextMenu";
import { useWorkspace } from "../workspaces/context";
import { activate } from "./panels";
import { tabMenu } from "./tabMenu";

/** The default tab, except closing an editor with unsaved edits asks first. Right-click
 * selects it and opens its menu. */
export function Tab(props: IDockviewPanelHeaderProps) {
  const { api, containerApi } = props;
  const workspace = useWorkspace();
  async function close() {
    if (await confirmClose(workspace, api.id)) api.close();
  }
  return (
    <DockviewDefaultTab
      {...props}
      closeActionOverride={close}
      onContextMenu={(e) => {
        const panel = containerApi.getPanel(api.id);
        if (!panel) return;
        // Active first, so the review tab's items act on its own review.
        activate(panel);
        const inBottom = !!e.currentTarget.closest(".bottom-panel");
        openMenu(e, tabMenu({ workspace, api: containerApi, panel, inBottom, close }));
      }}
    />
  );
}
