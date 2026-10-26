import type { DockviewApi, SerializedDockview } from "dockview-react";
import { logger } from "../core/log";
import { dropStalePanels, reshowActive } from "../core/panels";
import type { Fold } from "./fold";

const log = logger("layout");

/** A workspace's saved docks. The bottom one is left out while it has no panels. */
export interface SavedLayout {
  main?: SerializedDockview;
  bottom?: SerializedDockview;
  /** Each dock's collapsed groups, by group id. */
  folds?: { main?: Record<string, Fold>; bottom?: Record<string, Fold> };
}

/** Reads a saved layout. Layouts from earlier builds are the main dock's alone. */
export function parseLayout(saved: string | null): SavedLayout {
  if (!saved) return {};
  try {
    const json = JSON.parse(saved);
    return "main" in json || "bottom" in json ? json : { main: json };
  } catch (e) {
    log.warn("discarding unreadable layout", e);
    return {};
  }
}

/** Restores `saved` into `api`, or calls `fallback` when that leaves it without panels. */
export function restoreLayout(
  api: DockviewApi,
  saved: SerializedDockview | undefined,
  fallback?: () => void,
) {
  if (saved) {
    try {
      api.fromJSON(saved);
      reshowActive(api);
      dropStalePanels(api);
      // Report tabs saved under their old name.
      for (const p of api.panels) if (p.title === "Reviewers") p.api.setTitle("Report");
      if (api.panels.length > 0) return;
    } catch (e) {
      log.warn("discarding unreadable layout", e);
      api.clear();
    }
  }
  fallback?.();
}
