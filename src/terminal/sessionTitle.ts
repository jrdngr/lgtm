import type { IDockviewPanel } from "dockview-react";
import { commands } from "../ipc";
import { terminalTitle } from "./registry";
import { isAgentTitle, titleLabel } from "./title";

/** Names terminal tab `panel` after its Claude Code session: the name given with `/rename`,
 * else the title Claude Code shows. A name the user typed on the tab gives way. */
export async function refreshName(panel: IDockviewPanel) {
  const named = await commands.terminalSessionTitle(panel.id);
  const shown = terminalTitle(panel.id);
  const title = named ?? (shown && isAgentTitle(shown) ? titleLabel(shown) : undefined);
  panel.api.updateParameters({ sessionTitle: named ?? undefined, renamed: false });
  if (title) panel.api.setTitle(title);
}
