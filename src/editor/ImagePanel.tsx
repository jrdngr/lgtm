import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useState } from "react";
import { commands, errorText, unwrap } from "../ipc";
import { useWorkspace } from "../workspaces/context";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import { imageType } from "./image";
import type { EditorParams } from "./open";

/** An image file at its own size, scaled down when it doesn't fit the tab. Reloads when
 * the repo changes. */
export function ImagePanel({ params }: IDockviewPanelProps<EditorParams>) {
  const workspace = useWorkspace();
  const { path } = params;
  const [url, setUrl] = useState<string>();
  const [error, setError] = useState<string>();
  const [size, setSize] = useState<[number, number]>();

  const load = useCallback(
    () =>
      unwrap(commands.readImage(workspace, path)).then(
        (data) => {
          const next = `data:${imageType(path)};base64,${data}`;
          setUrl((u) => (u === next ? u : next));
          setError(undefined);
        },
        (e) => setError(errorText(e)),
      ),
    [workspace, path],
  );
  useEffect(() => {
    load();
  }, [load]);
  useRepoChanged(load);

  return (
    <div className="editor-panel">
      <div className="editor-path">
        <span className="path" title={path}>
          {path}
        </span>
        {size && (
          <span className="image-size">
            {size[0]} × {size[1]}
          </span>
        )}
      </div>
      {error && <div className="editor-bar error">{error}</div>}
      {url && (
        <div className="image-view">
          <img
            src={url}
            alt={path}
            draggable={false}
            onLoad={(e) => setSize([e.currentTarget.naturalWidth, e.currentTarget.naturalHeight])}
          />
        </div>
      )}
    </div>
  );
}
