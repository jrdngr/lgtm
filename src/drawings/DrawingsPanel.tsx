import { useEffect, useState } from "react";
import type { DrawingDto } from "../bindings";
import { dockOf } from "../core/docks";
import { openDrawing } from "../core/panels";
import { useWorkspace } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import { contextMenu } from "../ui/contextMenu";
import { confirmAction, promptText } from "../ui/prompt";
import { ago } from "../ui/time";
import { drawingsChanged, onNewDrawing, useDrawings } from "./store";
import "./drawings.css";

const tagCount = (d: DrawingDto) => Object.keys(d.canvas.views?.main?.tags ?? {}).length;

/** The repo's drawings: a row opens one, New names and opens a fresh one. */
export function DrawingsPanel() {
  const workspace = useWorkspace();
  const { items, error: loadError } = useDrawings();
  const [naming, setNaming] = useState<string>();
  const [error, setError] = useState<string>();
  useEffect(() => onNewDrawing(() => setNaming("")));

  const open = (d: DrawingDto) => {
    const api = dockOf(workspace);
    if (api) openDrawing(api, d.id, d.name);
  };
  const act = (f: () => Promise<unknown>) =>
    f().then(
      () => {
        setError(undefined);
        drawingsChanged();
      },
      (e) => setError(errorText(e)),
    );
  const create = (name: string) => {
    setNaming(undefined);
    if (!name.trim()) return;
    act(async () => open(await unwrap(commands.createDrawing(name.trim()))));
  };
  const rename = async (d: DrawingDto) => {
    const name = await promptText("Rename Drawing", { value: d.name, action: "Rename" });
    if (name?.trim()) act(() => unwrap(commands.renameDrawing(d.id, name.trim())));
  };
  const remove = async (d: DrawingDto) => {
    if (!(await confirmAction(`Delete “${d.name}”?`, "Delete"))) return;
    dockOf(workspace)?.getPanel(`drawing-${d.id}`)?.api.close();
    act(() => unwrap(commands.deleteDrawing(d.id)));
  };

  return (
    <div className="dr-panel">
      <div className="dr-panel-head">
        <b>Drawings</b>
        <span className="spacer" />
        <button type="button" className="dr-new" onClick={() => setNaming("")}>
          New<kbd>⌘⇧X</kbd>
        </button>
      </div>
      {naming !== undefined && (
        <input
          className="dr-name"
          // biome-ignore lint/a11y/noAutofocus: New asks for the name right away
          autoFocus
          aria-label="Name"
          value={naming}
          onChange={(e) => setNaming(e.target.value)}
          onBlur={() => setNaming(undefined)}
          onKeyDown={(e) => {
            if (e.key === "Enter") create(naming);
            else if (e.key === "Escape") setNaming(undefined);
          }}
        />
      )}
      {items?.map((d) => {
        const tags = tagCount(d);
        return (
          <button
            key={d.id}
            type="button"
            className="dr-row"
            onClick={() => open(d)}
            onContextMenu={contextMenu(() => [
              { label: "Open", onSelect: () => open(d) },
              { label: "Rename…", onSelect: () => rename(d) },
              "-",
              { label: "Delete…", onSelect: () => remove(d) },
            ])}
          >
            <span className="nm">{d.name}</span>
            {tags > 0 && <span className="dr-tags">{tags === 1 ? "1 tag" : `${tags} tags`}</span>}
            {d.updatedAt != null && <span className="dr-when">{ago(d.updatedAt)}</span>}
          </button>
        );
      })}
      {(error || loadError) && <div className="review-error">{error || loadError}</div>}
    </div>
  );
}
