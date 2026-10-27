/** What an older build's right panel remembered under `key`, such as a tab's inner view. */
function savedSub(key: string): unknown {
  try {
    return JSON.parse(localStorage.getItem(key) ?? "null");
  } catch {
    return null;
  }
}

/** Right panel views from older builds, by the view that shows them now. */
const MOVED: Record<string, string> = {
  files: "changes",
  symbols: "changes",
  bots: "reviewers",
  design: "qa",
  pr: "comments",
  walkthrough: "comments",
  notes: "artifacts",
  drawings: "artifacts",
};

/** The right panel view for a view id saved by any build. Older builds nested Changes under
 * "diff", and Reports under "reports" next to a view that's now gone. */
export function movedView(view: string, saved: (key: string) => unknown = savedSub): string {
  if (view === "diff") return "changes";
  if (view === "reports") {
    return saved("lgtm:side:reports") === "reviewers" ? "reviewers" : "comments";
  }
  return MOVED[view] ?? view;
}
