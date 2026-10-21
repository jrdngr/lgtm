import { dockOf } from "../core/docks";
import { openArchitecture } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import { showCode } from "../design/designs";
import { workspaceStore } from "../review/workspaceStore";

/** A chunk the Architecture tab narrows its Review lens to. */
export interface ChunkFocus {
  title: string;
  paths: string[];
}

const store = workspaceStore<ChunkFocus>();

export const useChunkFocus = store.use;

/** Opens the Architecture tab with its Review lens on the chunk's files alone. */
export function showChunk(workspace: WorkspaceName, focus: ChunkFocus) {
  store.publish(workspace, focus);
  showCode();
  const api = dockOf(workspace);
  if (api) openArchitecture(api);
}

export function clearChunk(workspace: WorkspaceName) {
  store.retract(workspace, store.get(workspace));
}
