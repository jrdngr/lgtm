import { Panel, useReactFlow } from "@xyflow/react";
import { IconButton } from "../ui/icons";

/** Zoom in, zoom out and fit, floating at the canvas's bottom right. */
export function ZoomControls() {
  const flow = useReactFlow();
  return (
    <Panel position="bottom-right" className="gzoom">
      <IconButton icon="zoomIn" label="Zoom in" onClick={() => flow.zoomIn()} />
      <IconButton icon="zoomOut" label="Zoom out" onClick={() => flow.zoomOut()} />
      <IconButton icon="fit" label="Fit" onClick={() => flow.fitView({ padding: 0.1 })} />
    </Panel>
  );
}
