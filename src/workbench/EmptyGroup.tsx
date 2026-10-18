import type { IWatermarkPanelProps } from "dockview-react";
import { addToGroup, KINDS, kindTitle } from "../core/panels";
import "./GroupActions.css";

/** An empty pane: the kinds of tab to open in it, and a way to close it if it's not the last. */
export function EmptyGroup({ containerApi, group }: IWatermarkPanelProps) {
  return (
    <div className="placeholder empty-group">
      <div className="empty-group-kinds">
        {KINDS.map((kind) => (
          <button
            type="button"
            key={kind}
            onClick={() => {
              const target =
                containerApi.groups.find((g) => g.id === group?.id) ?? containerApi.addGroup();
              addToGroup(containerApi, target, kind);
            }}
          >
            {kindTitle(kind)}
          </button>
        ))}
      </div>
      {group && containerApi.groups.length > 1 && (
        <button type="button" onClick={() => group.api.close()}>
          Close pane
        </button>
      )}
    </div>
  );
}
