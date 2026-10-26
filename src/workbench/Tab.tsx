import { DockviewDefaultTab, type IDockviewPanelHeaderProps } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import { isMac } from "../core/keys";
import { activate } from "../core/panels";
import { useWorkspace } from "../core/workspace";
import { confirmClose } from "../editor/unsaved";
import { useAtOutside } from "../review/workingCopy";
import { hasTerminal, useRoles } from "../terminal/registry";
import { openMenu } from "../ui/contextMenu";
import { Icon } from "../ui/icons";
import { Glyph } from "../workspaces/AgentGlyph";
import { tabState, useActivity } from "../workspaces/activity";
import { fitTabs } from "./tabFit";
import { tabMenu } from "./tabMenu";
import { clearPicks, togglePick, useTabPicked } from "./tabPick";
import { setRenaming, useRenaming } from "./tabRename";

// Dockview's pointer drags start on any button, and the native context menu swallows the
// pointerup, so a right-click would leave the tab following the pointer.
window.addEventListener(
  "pointerdown",
  (e) => {
    if (!(e.target instanceof Element)) return;
    // The tab's own buttons click without moving it, and ⌘-click picks a tab without showing
    // or dragging it.
    const tab = e.target.closest(".dv-tab");
    if (e.target.closest(".tab-rename") || (tab && (e.button !== 0 || pickKey(e)))) {
      e.stopPropagation();
    }
  },
  true,
);

const pickKey = (e: MouseEvent | React.MouseEvent) => (isMac ? e.metaKey : e.ctrlKey);

/** The tab's title as a text field: Enter or leaving it renames, Escape keeps the title. */
function RenameField({ title, onDone }: { title: string; onDone(title?: string): void }) {
  const done = useRef(false);
  const field = useRef<HTMLInputElement>(null);
  // After the menu closes and the tab takes focus.
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      field.current?.focus();
      field.current?.select();
    });
    return () => cancelAnimationFrame(frame);
  }, []);
  const finish = (value?: string) => {
    if (done.current) return;
    done.current = true;
    onDone(value);
  };
  return (
    <input
      className="tab-rename"
      defaultValue={title}
      aria-label="Tab title"
      spellCheck={false}
      size={Math.max(4, title.length + 1)}
      ref={field}
      onBlur={(e) => finish(e.currentTarget.value)}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") finish(e.currentTarget.value);
        else if (e.key === "Escape") finish();
      }}
    />
  );
}

/** The default tab, except closing an editor with unsaved edits asks first. Right-click
 * selects it and opens its menu. A terminal's tab leads with a ›_, teal on the interactive
 * pin and violet on the background one, whose titles take the same colors. A tab running
 * agents shows their most urgent state before its title. ⌘-click picks tabs to drag together. */
export function Tab(props: IDockviewPanelHeaderProps) {
  const { api, containerApi } = props;
  const workspace = useWorkspace();
  const renaming = useRenaming(api.id);
  const role = useRoles(workspace)(api.id);
  const picked = useTabPicked(api.id);
  const [title, setTitle] = useState(api.title ?? "");
  useEffect(() => {
    // A tab coming, going or retitled changes how much room its pane's titles have.
    fitTabs();
    const sub = api.onDidTitleChange((e) => {
      setTitle(e.title);
      fitTabs();
    });
    return () => {
      sub.dispose();
      fitTabs();
    };
  }, [api]);
  const panel = containerApi.getPanel(api.id);
  const agents = tabState(useActivity(workspace), api.id);
  const atOutside = useAtOutside(workspace);
  const [shown, setShown] = useState(api.isVisible);
  useEffect(() => {
    const sub = api.onDidVisibilityChange((e) => setShown(e.isVisible));
    return () => sub.dispose();
  }, [api]);
  // Saving the shown editor writes to `@`, which is outside the review.
  const wdot = atOutside && shown && panel?.view.contentComponent === "editor";
  const glyph = agents ? (
    <Glyph state={agents} className="tab-glyph" />
  ) : (
    wdot && <span className="tab-wdot" />
  );
  async function close() {
    if (await confirmClose(workspace, api.id)) api.close();
  }
  const tab = renaming ? (
    <div className="dv-default-tab">
      <RenameField
        title={api.title ?? ""}
        onDone={(title) => {
          if (title?.trim()) {
            api.setTitle(title.trim());
            api.updateParameters({ renamed: true });
          }
          setRenaming(null);
        }}
      />
    </div>
  ) : (
    <DockviewDefaultTab
      {...props}
      data-send={role}
      data-picked={picked || undefined}
      title={title}
      onClick={(e) => {
        if (pickKey(e)) togglePick(api.id, api.group.activePanel?.id);
        else clearPicks();
      }}
      closeActionOverride={close}
      onContextMenu={(e) => {
        if (!panel) return;
        // Active first, so the review tab's items act on its own review.
        activate(panel);
        const inBottom = !!e.currentTarget.closest(".bottom-panel");
        openMenu(e, tabMenu({ workspace, api: containerApi, panel, inBottom, close }));
      }}
    />
  );
  if (!hasTerminal(api.id))
    return glyph ? (
      <div className="tab-row">
        {glyph}
        {tab}
      </div>
    ) : (
      tab
    );
  return (
    <div className="tab-row">
      <span className={`tab-pin${role ? ` ${role}` : ""}`}>
        <Icon name="terminal" size={13} />
      </span>
      {glyph}
      {tab}
    </div>
  );
}
