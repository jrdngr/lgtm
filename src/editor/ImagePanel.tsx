import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useState } from "react";
import { commands, errorText, unwrap } from "../ipc";
import { saveTabParams } from "../workbench/tabParams";
import { useWorkspace } from "../workspaces/context";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import { imageType } from "./image";
import type { EditorParams } from "./open";
import { useImageZoom } from "./useImageZoom";

/** An image file, scaled down to fit the tab until zoomed. Reloads when the repo changes. */
export function ImagePanel({ params, api }: IDockviewPanelProps<EditorParams>) {
  const workspace = useWorkspace();
  const { path } = params;
  const [url, setUrl] = useState<string>();
  const [error, setError] = useState<string>();

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
  const { controls, view, size } = useImageZoom(url, path, params.zoom, (zoom) =>
    saveTabParams(api, { zoom: zoom === "shrink" ? undefined : zoom }),
  );

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
        {url && controls}
      </div>
      {error && <div className="editor-bar error">{error}</div>}
      {view}
    </div>
  );
}
