import { DockviewDefaultTab, type IDockviewPanelHeaderProps } from "dockview-react";
import { confirmClose } from "../editor/unsaved";

/** The default tab, except closing an editor with unsaved edits asks first. */
export function Tab(props: IDockviewPanelHeaderProps) {
  const { api } = props;
  async function close() {
    if (await confirmClose(api.id, api.title ?? api.id)) api.close();
  }
  return <DockviewDefaultTab {...props} closeActionOverride={close} />;
}
