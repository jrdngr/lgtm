import { useMemo } from "react";
import type { DesignDto } from "../bindings";
import type { CanvasStore } from "../canvas/useCanvas";
import { commands, unwrap } from "../ipc";

/** Where a design's drawing over its map is stored. */
export function useDesignCanvas(design: DesignDto): CanvasStore {
  const { id, canvas } = design;
  return useMemo(
    () => ({
      stored: canvas,
      save: (c) => unwrap(commands.saveDesignCanvas(id, c)),
      undoAgent: () => unwrap(commands.undoDesignAgent(id)).then((d) => d.canvas),
    }),
    [id, canvas],
  );
}
