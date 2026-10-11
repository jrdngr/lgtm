import { useEffect, useState } from "react";
import type { GraphDto } from "../bindings";
import { commands, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";

/** Loads by the review's files, so the panels showing one review share a load, and a refresh
 * that changes the files loads again. */
const loads = new WeakMap<object, Promise<GraphDto>>();

function load(id: number, files: object) {
  let p = loads.get(files);
  if (!p) {
    p = unwrap(commands.reviewGraph(id));
    loads.set(files, p);
    p.catch(() => loads.delete(files));
  }
  return p;
}

interface State {
  files?: object;
  graph?: GraphDto;
  error?: string;
}

/** The current review's symbol graph. */
export function useGraph(review: CurrentReview | undefined) {
  const [state, setState] = useState<State>({});
  const id = review?.id;
  const files = review?.files;
  useEffect(() => {
    if (id === undefined || !files) return;
    let live = true;
    load(id, files).then(
      (graph) => live && setState({ files, graph }),
      (e) => live && setState({ files, error: e instanceof Error ? e.message : String(e) }),
    );
    return () => {
      live = false;
    };
  }, [id, files]);
  const fresh = state.files === files;
  return {
    graph: fresh ? state.graph : undefined,
    error: fresh ? state.error : undefined,
    loading: !!files && !fresh,
  };
}
