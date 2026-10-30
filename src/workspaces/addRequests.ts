import { requests } from "../core/request";

/** What the Workspaces panel's + New menu starts. */
export type AddKind = "workspace" | "pr";

const adds = requests<AddKind>();

/** Asks the Workspaces panel to start adding a workspace or a PR, as its shortcuts do. */
export const requestAdd = adds.send;
export const onAddRequest = adds.listen;

/** What the Workspaces panel asks about a row for another repo's panel. */
export interface RowAsk {
  kind: "delete" | "locate";
  ws: string;
}

const asks = requests<RowAsk>();

/** Asks the Workspaces panel to confirm a workspace's Delete, or to ask where its files are. */
export const requestAsk = asks.send;
export const onAsk = asks.listen;
