import { openUrl } from "../ui/pathMenu";
import type { WorkspaceName } from "../workspaces/context";
import { openFile } from "./open";

/** A link in a line of markdown: where it sits and where it goes. */
export interface LineLink {
  from: number;
  to: number;
  href: string;
}

/** `[text](href)`, `<href>`, or a bare web address. */
const LINK =
  /\[[^\]\n]*\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)|<((?:https?|mailto):[^>\s]+)>|\b(https?:\/\/[^\s<>()]+[^\s<>().,;:!?'"])/g;

/** The links in a line of markdown, in order. */
export function linksIn(line: string): LineLink[] {
  return [...line.matchAll(LINK)].map((m) => ({
    from: m.index,
    to: m.index + m[0].length,
    href: m[1] ?? m[2] ?? m[3],
  }));
}

/** Where a link in the file at `from` (repo-relative) goes: a web address, or a file in the
 * repo. Links within the page go nowhere. */
export function resolveLink(
  href: string,
  from: string | undefined,
): { url: string } | { path: string } | undefined {
  if (/^(https?|mailto):/i.test(href)) return { url: href };
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("#") || from === undefined) return;
  const target = decodeURI(href.split("#")[0]);
  const parts = target.startsWith("/") ? [] : from.split("/").slice(0, -1);
  for (const part of target.split("/")) {
    if (part === "..") parts.pop();
    else if (part && part !== ".") parts.push(part);
  }
  return parts.length ? { path: parts.join("/") } : undefined;
}

/** Opens a link in the file at `from`: web addresses in the browser, files in an editor. */
export function followLink(href: string, workspace: WorkspaceName, from?: string) {
  const to = resolveLink(href, from);
  if (!to) return;
  if ("url" in to) openUrl(to.url);
  else openFile(workspace, to.path);
}
