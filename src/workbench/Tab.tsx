import { DockviewDefaultTab, type IDockviewPanelHeaderProps } from "dockview-react";
import { useEffect, useRef } from "react";
import { confirmClose } from "../editor/unsaved";
import { hasTerminal, pinTarget, useSendTarget } from "../terminal/registry";
import { openMenu } from "../ui/contextMenu";
import { Icon } from "../ui/icons";
import { Glyph } from "../workspaces/AgentGlyph";
import { tabState, useActivity } from "../workspaces/activity";
import { useWorkspace } from "../workspaces/context";
import { activate, isReviewPanel } from "./panels";
import { tabMenu } from "./tabMenu";
import { setRenaming, useRenaming } from "./tabRename";

// Dockview's pointer drags start on any button, and the native context menu swallows the
// pointerup, so a right-click would leave the tab following the pointer.
window.addEventListener(
  "pointerdown",
  (e) => {
    if (!(e.target instanceof Element)) return;
    // The tab's own buttons click without moving it.
    if (
      e.target.closest(".tab-pin, .tab-rename") ||
      (e.button !== 0 && e.target.closest(".dv-tab"))
    ) {
      e.stopPropagation();
    }
  },
  true,
);

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
 * selects it and opens its menu. The send target's tab is marked, and a terminal's tab leads
 * with a ›_ that pins it as the send target, shown while the tab is selected. A tab running
 * agents shows their most urgent state before its title. */
export function Tab(props: IDockviewPanelHeaderProps) {
  const { api, containerApi } = props;
  const workspace = useWorkspace();
  const send = useSendTarget(workspace);
  const renaming = useRenaming(api.id);
  const pinned = send.pinned === api.id;
  const mark = pinned ? "pinned" : send.target === api.id ? "target" : undefined;
  const panel = containerApi.getPanel(api.id);
  const agents = tabState(useActivity(workspace), api.id, !!panel && isReviewPanel(panel));
  const glyph = agents && <Glyph state={agents} className="tab-glyph" />;
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
      data-send={mark}
      closeActionOverride={close}
      onContextMenu={(e) => {
        const panel = containerApi.getPanel(api.id);
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
      <button
        type="button"
        className={`tab-pin${pinned ? " on" : ""}`}
        aria-pressed={pinned}
        aria-label={pinned ? "Unpin Send Target" : "Pin as Send Target"}
        title={pinned ? "Unpin Send Target" : "Pin as Send Target"}
        onClick={() => pinTarget(workspace, pinned ? null : api.id)}
      >
        <Icon name="terminal" size={13} />
      </button>
      {glyph}
      {tab}
    </div>
  );
}
