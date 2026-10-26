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

/** What the Workspaces panel asks about a row for another repo's panel. */
export interface RowAsk {
  kind: "delete" | "locate";
  ws: string;
}

const askers = new Set<(ask: RowAsk) => void>();

/** Asks the Workspaces panel to confirm a workspace's Delete, or to ask where its files are. */
export function requestAsk(ask: RowAsk) {
  for (const l of askers) l(ask);
}

export function onAsk(f: (ask: RowAsk) => void) {
  askers.add(f);
  return () => {
    askers.delete(f);
  };
}
