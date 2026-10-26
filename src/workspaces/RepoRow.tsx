import type { ProjectRepo } from "../bindings";
import type { ShellState } from "../frame/shell";
import { shell } from "../frame/shell";
import { type CtxEntry, openMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { Glyph, Moon } from "./AgentGlyph";
import { type Activity, AGENT_STATES } from "./activity";
import { COLORS } from "./pins";
import { colorItems } from "./workspaceMenu";

/** The most urgent agent state of the repo's workspaces. */
export function rollup(s: ShellState, label: string): Activity["state"] {
  const tree = s.trees[label];
  const states = [...(tree?.top ?? []), ...(tree?.rest ?? [])].map((r) => r.a.state);
  const ranked = AGENT_STATES.filter((st) => states.includes(st));
  return ranked[0] ?? "none";
}

interface Props {
  repo: ProjectRepo;
  s: ShellState;
  onError(e: string): void;
}

/** A repo in the project's tree: a chevron, its name, a ⋯ menu, and, collapsed, its
 * workspaces' most urgent agent state. Clicking it folds its workspaces away. */
export function RepoRow({ repo, s, onError }: Props) {
  const sh = shell();
  const { label, asleep } = repo;
  const collapsed = asleep || s.ui.collapsed.includes(label);
  const color = s.ui.colors[label];
  const pinned = s.ui.pinned.includes(label);
  const awake = s.project.repos.filter((r) => !r.asleep).length;
  const report = (p: Promise<string | undefined> | undefined) => p?.then((e) => e && onError(e));

  const items = (): CtxEntry[] => [
    { label: pinned ? "Unpin" : "Pin to Top", onSelect: () => sh?.togglePin(label) },
    { label: "Color", items: colorItems(color, (c) => sh?.setColor(label, c)) },
    "-",
    !asleep && {
      label: collapsed ? "Expand" : "Collapse",
      onSelect: () => sh?.toggleCollapsed(label),
    },
    asleep
      ? { label: "Wake", onSelect: () => report(sh?.wake(label)) }
      : { label: "Sleep", disabled: awake <= 1, onSelect: () => report(sh?.sleep(label)) },
    "-",
    {
      label: "Remove from Project",
      disabled: s.project.repos.length <= 1,
      onSelect: () => report(sh?.remove(label)),
    },
  ];
  const state = collapsed ? rollup(s, label) : "none";

  return (
    <li
      className={`workspace repo-row${asleep ? " asleep" : ""}${color ? " colored" : ""}`}
      style={color ? ({ "--ws-color": COLORS[color] } as React.CSSProperties) : undefined}
      onContextMenu={(e) => openMenu(e, items())}
    >
      <button
        type="button"
        className="workspace-open"
        aria-expanded={!collapsed}
        onClick={() => (asleep ? report(sh?.wake(label)) : sh?.toggleCollapsed(label))}
      >
        <span className="chevron">{collapsed ? "▸" : "▾"}</span>
        <span className="workspace-title">
          <span className="ellipsis">{repo.name}</span>
        </span>
      </button>
      <span className="workspace-side">
        <div className="menu reveal">
          <IconButton
            icon="more"
            label="Repository actions"
            onClick={(e) => openMenu(e, items())}
          />
        </div>
        {asleep ? (
          <Moon />
        ) : (
          <span className={`ws-agents${state === "none" ? " hold" : ""}`}>
            <Glyph state={state} />
          </span>
        )}
      </span>
    </li>
  );
}
