import { workspaceKey } from "../core/workspace";
import { shell, type TreeAgent, type TreeRow } from "../frame/shell";
import { openMenu, tidy } from "../ui/contextMenu";
import { Menu, type MenuItem } from "../ui/Menu";
import { AgentGlyph, Moon } from "./AgentGlyph";
import { COLORS } from "./pins";
import { Lines } from "./WorkspaceRow";

/** A workspace of another repo in the project, drawn from what that repo's frame sent. Its
 * actions go to that frame. */
export function ForeignRow({ row, label }: { row: TreeRow; label: string }) {
  const s = shell();
  const { w, color } = row;
  const frame = () => s?.frame(label);
  /** The ⋯ menu: the Sleep or Wake and Delete of the row's own menu, from its repo's frame. */
  const items = () =>
    tidy(frame()?.menu(w.name) ?? []).filter(
      (i): i is MenuItem =>
        i !== "-" && !("items" in i) && ["Sleep", "Wake", "Delete Workspace…"].includes(i.label),
    );
  return (
    <li
      className={`workspace nested${w.asleep ? " asleep" : ""}${color ? " colored" : ""}`}
      style={color ? ({ "--ws-color": COLORS[color] } as React.CSSProperties) : undefined}
      data-row={`${label}/${workspaceKey(w.name)}`}
      onContextMenu={(e) => openMenu(e, frame()?.menu(w.name) ?? [])}
    >
      <button
        type="button"
        className="workspace-open"
        onClick={() => s?.show(label, w.name)}
        title={w.description.trim() || undefined}
      >
        <Lines w={w} pr={row.pr} />
      </button>
      <span className="workspace-side">
        <Menu className="reveal" label="Workspace actions" items={items()} />
        {w.asleep ? (
          <Moon />
        ) : (
          <AgentGlyph
            a={row.a}
            name={(g) => (g as TreeAgent).name}
            role={(g) => (g as TreeAgent).role}
            onOpen={(g) => {
              s?.show(label);
              frame()?.openAgent(w.name, g);
            }}
          />
        )}
      </span>
    </li>
  );
}
