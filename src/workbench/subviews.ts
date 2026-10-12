import { remembered } from "./remembered";

/** What the right panel's Diff tab lists. */
export const diffView = remembered("lgtm:side:diff", "files", ["files", "symbols"]);

/** Which report the right panel's Reports tab shows. */
export const reportView = remembered("lgtm:side:reports", "walkthrough", [
  "walkthrough",
  "reviewers",
]);

/** Whether the Comments tab's PR section is open. */
export const prOpen = remembered("lgtm:side:pr", true, [true, false]);
