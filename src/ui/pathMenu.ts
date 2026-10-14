import { message } from "@tauri-apps/plugin-dialog";
import { revealInFiles } from "../editor/reveal";
import { commands, errorText, unwrap } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import { workspaceRoot } from "../workspaces/useWorkspaces";
import { type CtxEntry, copy } from "./contextMenu";

/** Shows what went wrong with a menu action that can fail. */
export function run(p: Promise<unknown>) {
  p.catch((e) => message(errorText(e), { kind: "error" }));
}

/** `rel` as an absolute path, or `rel` itself before the workspace list loads or when it
 * already is one (personas and prompts). */
export function absolutePath(ws: WorkspaceName, rel: string): string {
  const root = workspaceRoot(ws);
  if (rel.startsWith("/") || !root) return rel;
  return rel ? `${root}/${rel}` : root;
}

/** Copy Path, Copy Relative Path and Reveal in Finder for a workspace file or folder, plus
 * Reveal in Files with `files`. */
export function pathItems(
  ws: WorkspaceName,
  rel: string,
  opts: { relative?: boolean; files?: boolean } = {},
): CtxEntry[] {
  return [
    { label: "Copy Path", onSelect: () => copy(absolutePath(ws, rel)) },
    opts.relative !== false &&
      !rel.startsWith("/") && { label: "Copy Relative Path", onSelect: () => copy(rel) },
    opts.files && { label: "Reveal in Files", onSelect: () => revealInFiles(ws, rel) },
    { label: "Reveal in Finder", onSelect: () => reveal(ws, rel) },
  ];
}

/** Opens a web page in the browser. */
export const openUrl = (url: string) => run(unwrap(commands.openUrl(url)));

export const reveal = (ws: WorkspaceName, rel: string) => run(unwrap(commands.revealPath(ws, rel)));
