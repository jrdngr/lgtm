/** What the Workspaces panel's + New menu starts. */
export type AddKind = "workspace" | "pr";

const listeners = new Set<(kind: AddKind) => void>();

/** Asks the Workspaces panel to start adding a workspace or a PR, as its shortcuts do. */
export function requestAdd(kind: AddKind) {
  for (const l of listeners) l(kind);
}

export function onAddRequest(f: (kind: AddKind) => void) {
  listeners.add(f);
  return () => {
    listeners.delete(f);
  };
}
