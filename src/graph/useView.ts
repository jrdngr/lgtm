import { useMemo, useState } from "react";
import type { GraphDto } from "../bindings";
import type { Tree } from "./tree";
import { firstView, type View } from "./view";

/** Each review's last view, so reopening the graph or reloading the review keeps what was
 * open. Callers shown are by node id, so they're kept only for the same graph. */
const saved = new Map<string, { graph: GraphDto; view: View }>();

/** The graph's view for review `key`: the saved one, or the first view. */
export function useView(key: string, graph: GraphDto, tree: Tree) {
  const [state, setState] = useState<{ key: string; graph: GraphDto; view: View }>();
  // Kept stable, since a new view lays the graph out again.
  const fallback = useMemo<View>(() => {
    const last = saved.get(key);
    return last
      ? {
          expanded: last.view.expanded,
          callersOpen: last.graph === graph ? last.view.callersOpen : new Set(),
        }
      : firstView(graph, tree);
  }, [key, graph, tree]);
  const view = state?.key === key && state.graph === graph ? state.view : fallback;
  const setView = (next: View) => {
    saved.set(key, { graph, view: next });
    setState({ key, graph, view: next });
  };
  return [view, setView] as const;
}
