/** What an older build's right panel remembered under `key`, such as a tab's inner view. */
function savedSub(key: string): unknown {
  try {
    return JSON.parse(localStorage.getItem(key) ?? "null");
  } catch {
    return null;
  }
}

/** Right panel views from older builds, by the view that shows them now. */
const MOVED: Record<string, string> = { files: "changes", bots: "reviewers", design: "qa" };

/** The right panel view for a view id saved by any build. Older builds nested Files and
 * Symbols under "diff", and Walkthrough and Reviewers under "reports", each remembering the
 * one it showed. */
export function movedView(view: string, saved: (key: string) => unknown = savedSub): string {
  if (view === "diff") return saved("lgtm:side:diff") === "symbols" ? "symbols" : "changes";
  if (view === "reports") {
    return saved("lgtm:side:reports") === "reviewers" ? "reviewers" : "walkthrough";
  }
  return MOVED[view] ?? view;
}
