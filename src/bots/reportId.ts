import type { BotRun } from "../bindings";
import { commands } from "../ipc";
import { copy } from "../ui/contextMenu";
import type { MenuItem } from "../ui/Menu";
import { promptText } from "../ui/prompt";
import { refreshRun } from "./runs";

/** The id the user tells agents: the report's name, or `R<id>`. */
export const reportLabel = (run: BotRun) => run.name ?? `R${run.id}`;

/** Asks for a new id. Its `R<id>` brings the default back. */
async function rename(run: BotRun, onError: (message: string) => void) {
  const name = await promptText("Rename report", { value: reportLabel(run), action: "Rename" });
  if (name === undefined) return;
  const r = await commands.renameReport(run.id, name);
  if (r.status === "error") return onError(r.error);
  await refreshRun(run.id);
}

/** Rename… and Copy Report ID. */
export function reportIdItems(run: BotRun, onError: (message: string) => void): MenuItem[] {
  return [
    { label: "Rename…", onSelect: () => rename(run, onError) },
    { label: "Copy Report ID", onSelect: () => copy(reportLabel(run)) },
  ];
}
