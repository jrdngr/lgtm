import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const heard: ((e: { payload: { snapshot: boolean } }) => void)[] = [];
vi.mock("@tauri-apps/api/webviewWindow", () => ({ getCurrentWebviewWindow: () => ({}) }));
vi.mock("../bindings", () => ({
  events: {
    repoChanged: () => ({
      listen: (f: (e: { payload: { snapshot: boolean } }) => void) => heard.push(f),
    }),
  },
}));

const { onRepoChanged, showWorkspace } = await import("./useRepoChanged");

/** A burst of repo-changed reports, as the watchers send them, then the quiet after. */
function changed(...snapshots: boolean[]) {
  for (const f of heard)
    for (const snapshot of snapshots.length ? snapshots : [false, false]) {
      f({ payload: { snapshot } });
    }
  vi.advanceTimersByTime(300);
}

describe("onRepoChanged", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("reloads the shown workspace's views at once and a hidden one's when it shows", () => {
    showWorkspace("a");
    const [a, b, any] = [vi.fn(), vi.fn(), vi.fn()];
    const stops = [onRepoChanged(a, "a"), onRepoChanged(b, "b"), onRepoChanged(any)];

    changed();
    changed();
    expect(a).toHaveBeenCalledTimes(2);
    expect(any).toHaveBeenCalledTimes(2);
    expect(b).not.toHaveBeenCalled();

    showWorkspace("b");
    expect(b).toHaveBeenCalledTimes(1);
    showWorkspace("a");
    showWorkspace("b");
    expect(b).toHaveBeenCalledTimes(1);
    for (const stop of stops) stop();
  });

  it("drops a hidden view's catch-up once it stops listening", () => {
    showWorkspace("a");
    const b = vi.fn();
    const stop = onRepoChanged(b, "b");
    changed();
    stop();
    showWorkspace("b");
    expect(b).not.toHaveBeenCalled();
  });

  it("skips the app's own snapshots for views that ask to", () => {
    showWorkspace("a");
    const [pills, review] = [vi.fn(), vi.fn()];
    const stops = [onRepoChanged(pills, "a", { snapshots: false }), onRepoChanged(review, "a")];
    changed(true, true);
    expect(pills).not.toHaveBeenCalled();
    expect(review).toHaveBeenCalledTimes(1);
    changed(true, false);
    expect(pills).toHaveBeenCalledTimes(1);
    for (const stop of stops) stop();
  });
});
