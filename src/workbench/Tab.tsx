import { DockviewDefaultTab, type IDockviewPanelHeaderProps } from "dockview-react";
import { confirmClose } from "../editor/unsaved";
import { useWorkspace } from "../workspaces/context";

/** The default tab, except closing an editor with unsaved edits asks first. */
export function Tab(props: IDockviewPanelHeaderProps) {
  const { api } = props;
  const workspace = useWorkspace();
  async function close() {
    if (await confirmClose(workspace, api.id)) api.close();
  }
  return <DockviewDefaultTab {...props} closeActionOverride={close} />;
}
