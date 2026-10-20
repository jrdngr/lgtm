import { useState } from "react";
import { dockOf } from "../core/docks";
import { openArchitecture } from "../core/panels";
import { useWorkspace } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { contextMenu } from "../ui/contextMenu";
import { GroupHead } from "../ui/GroupHead";
import { designsChanged, editDesign, showCode, showDesign, useDesigns, useSource } from "./designs";
import { differsCount } from "./SourceSwitch";
import { useDesignGenerate } from "./useDesignGenerate";
import "./design.css";

/** The repo's designs in the Architecture sidebar: a row shows one, + makes one. */
export function DesignsSection({ onError }: { onError(e: string | undefined): void }) {
  const workspace = useWorkspace();
  const { items } = useDesigns(workspace);
  const { design: shown } = useSource();
  const [shut, setShut] = useState(false);
  const { generate, picker } = useDesignGenerate(workspace, onError);
  const show = (id: string) => {
    showDesign(id);
    const api = dockOf(workspace);
    if (api) openArchitecture(api);
  };
  const remove = async (id: string) => {
    try {
      await unwrap(commands.deleteDesign(id));
      if (shown === id) showCode();
      designsChanged();
    } catch (e) {
      onError(String(e));
    }
  };
  return (
    <>
      <div className="dsn-side-head">
        <GroupHead
          name="Designs"
          count={items?.length ?? 0}
          shut={shut}
          onToggle={() => setShut(!shut)}
        />
        <button
          type="button"
          className="dsn-plus"
          aria-label="New Design"
          onClick={() => editDesign(workspace)}
        >
          +
        </button>
      </div>
      {!shut &&
        items?.map(({ design, state }) => {
          const differs = differsCount(design.map);
          return (
            <button
              key={design.id}
              type="button"
              className={`dsn-side-row${design.id === shown ? " on" : ""}`}
              onClick={() => show(design.id)}
              onContextMenu={contextMenu(() => [
                { label: "Show", onSelect: () => show(design.id) },
                {
                  label: "Regenerate",
                  onSelect: () => {
                    show(design.id);
                    generate(design.id);
                  },
                },
                { label: "Edit…", onSelect: () => editDesign(workspace, design) },
                "-",
                { label: "Delete", onSelect: () => remove(design.id) },
              ])}
            >
              <span className="nm">{design.name}</span>
              {!!differs && <span className="dsn-drift">{differs} differs</span>}
              <span className={`dsn-dot ${state}`} />
            </button>
          );
        })}
      {picker}
    </>
  );
}
