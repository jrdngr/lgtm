import { workspaceKey } from "../core/workspace";
import { shell, type TreeAgent, type TreeRow } from "../frame/shell";
import { openMenu } from "../ui/contextMenu";
import { Menu } from "../ui/Menu";
import { AgentGlyph, Moon } from "./AgentGlyph";
import { COLORS } from "./pins";
import { Lines } from "./WorkspaceRow";
import { deleteItem } from "./workspaceMenu";

/** A workspace of another repo in the project, drawn from what that repo's frame sent. Its
 * actions go to that frame. */
export function ForeignRow({ row, label }: { row: TreeRow; label: string }) {
  const s = shell();
  const { w, color } = row;
  const frame = () => s?.frame(label);
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
        {w.name !== null && (
          <Menu
            className="reveal"
            label="Workspace actions"
            items={[deleteItem(w, () => w.name !== null && frame()?.askDelete(w.name))]}
          />
        )}
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
