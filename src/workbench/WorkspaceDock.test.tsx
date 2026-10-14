// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { file, review, rows } from "../review/fixture.test-utils";

vi.mock("@tauri-apps/api/webviewWindow", () => ({ getCurrentWebviewWindow: () => ({}) }));
vi.mock("../terminal/TerminalPanel", () => ({ TerminalPanel: () => null }));
// Every command succeeds: no saved layout, no PR, and a review of two files.
vi.mock("../bindings", async (actual) => {
  const ok = (data: unknown) => Promise.resolve({ status: "ok", data });
  let id = 0;
  const commands = new Proxy(
    {},
    {
      get: (_, name) => (_workspace: unknown, revset: string) => {
        if (name !== "openReview") return ok(null);
        const r = review([file("a.ts", [rows(["+x"])]), file("b.ts", [rows(["+y"])])]);
        return ok({ ...r, id: ++id, revset });
      },
    },
  );
  // An event is listened to directly or for a window: `e.listen` or `e(window).listen`.
  const listen = async () => () => {};
  const events = new Proxy({}, { get: () => Object.assign(() => ({ listen }), { listen }) });
  return { ...(await actual<object>()), commands, events };
});

globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

/** Lets loads and the renders they cause finish. */
async function settle() {
  for (let i = 0; i < 10; i++) await act(() => new Promise((r) => setTimeout(r)));
}

/** A workspace's dock with the Diff tab's file list beside it, as the workbench shows them. */
async function mount(workspace: string) {
  vi.resetModules();
  const { WorkspaceDock } = await import("./WorkspaceDock");
  const { FilesPanel } = await import("../review/FilesPanel");
  const { WorkspaceContext } = await import("../workspaces/context");
  const { dockOf } = await import("./docks");
  const { isReviewPanel } = await import("./panels");
  const { reopenClosed } = await import("./closedTabs");
  const el = document.createElement("div");
  document.body.append(el);
  await act(() =>
    createRoot(el).render(
      <WorkspaceContext.Provider value={workspace}>
        <WorkspaceDock workspace={workspace} hidden={false} setup={false} />
        <FilesPanel />
      </WorkspaceContext.Provider>,
    ),
  );
  await settle();
  const api = dockOf(workspace);
  if (!api) throw new Error("dock not registered");
  const reviews = () => api.panels.filter(isReviewPanel);
  const fileButton = (name: string) =>
    [...el.querySelectorAll<HTMLButtonElement>(".file-tree li button")].find(
      (b) => b.title === name,
    );
  return { el, api, reviews, fileButton, reopen: () => reopenClosed(workspace) };
}

beforeEach(() => {
  document.body.innerHTML = "";
});

describe("Diff file list", { timeout: 20_000 }, () => {
  it("opens one Review tab when none is open, on a double click too", async () => {
    const { api, reviews, fileButton } = await mount("a");
    await act(() => reviews()[0].api.close());
    await settle();
    expect(reviews()).toHaveLength(0);

    await act(() => {
      fileButton("b.ts")?.click();
      fileButton("b.ts")?.click();
    });
    await settle();
    expect(reviews()).toHaveLength(1);
    expect(api.activePanel).toBe(reviews()[0]);
  });

  it("shows a Review tab hidden behind another once the current one closes", async () => {
    const { api, el, reviews, fileButton } = await mount("b");
    const [first] = reviews();
    // A terminal pane below the review, then a second review behind the terminal.
    const terminal = api.addPanel({
      id: "terminal",
      component: "terminal",
      position: { referencePanel: first, direction: "below" },
    });
    const second = api.addPanel({
      id: "second",
      component: "review",
      position: { referencePanel: terminal },
    });
    await settle();
    await act(() => terminal.api.setActive());
    await act(() => first.api.setActive());
    await act(() => first.api.close());
    await settle();
    expect(el.querySelector(".placeholder")).toBeNull();

    await act(() => fileButton("b.ts")?.click());
    await settle();
    expect(reviews()).toEqual([second]);
    expect(second.api.isVisible).toBe(true);
  });
});

describe("Reopening closed tabs", { timeout: 20_000 }, () => {
  it("reopens closed tabs latest first, with their params", async () => {
    const { api, reviews, reopen } = await mount("r");
    const first = reviews()[0];
    const params = first.params;
    api.addPanel({ id: "second", component: "review", params: { ...params, revset: "@-" } });
    await settle();
    await act(() => first.api.close());
    await act(() => api.getPanel("second")?.api.close());
    await settle();
    expect(reviews()).toHaveLength(0);

    await act(() => reopen());
    expect(reviews().map((p) => [p.id, p.params?.revset])).toEqual([["second", "@-"]]);
    await act(() => reopen());
    expect(
      reviews()
        .map((p) => p.id)
        .sort(),
    ).toEqual([first.id, "second"].sort());
    expect(api.getPanel(first.id)?.params).toEqual(params);
  });
});

describe("Focusing a review pane", { timeout: 20_000 }, () => {
  it("keeps the review's content attached, so its scroll stays put", async () => {
    const { api, reviews } = await mount("f");
    const [review] = reviews();
    const terminal = api.addPanel({
      id: "terminal",
      component: "terminal",
      position: { referencePanel: review, direction: "below" },
    });
    await act(() => terminal.api.setActive());
    const content = review.view.content.element;
    const detached: Node[] = [];
    const observer = new MutationObserver((records) => {
      for (const r of records) detached.push(...r.removedNodes);
    });
    observer.observe(document.body, { childList: true, subtree: true });

    const { focusPanel } = await import("./panels");
    await act(() => focusPanel(review));
    await settle();
    observer.disconnect();
    expect(api.activePanel).toBe(review);
    expect(detached).not.toContain(content);
  });
});
