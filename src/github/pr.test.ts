import { describe, expect, it, vi } from "vitest";
import type { PrDto } from "../bindings";
import { reloadPr, setPr, usePr } from "./pr";

const prInfo = vi.fn();
vi.mock("../ipc", () => ({ commands: { prInfo: (...a: unknown[]) => prInfo(...a) } }));
vi.mock("react", async (actual) => ({
  ...(await actual<typeof import("react")>()),
  useEffect: () => {},
  useSyncExternalStore: (sub: (l: () => void) => void, get: () => unknown) => {
    sub(() => renders.push(get()));
    return get();
  },
}));
const renders: unknown[] = [];

const pr = (title: string) => ({ repo: "o/r", number: 7, title }) as unknown as PrDto;

describe("the workspace's PR", () => {
  it("changes only when the PR does, and keeps showing while it reloads", async () => {
    setPr("ws", pr("Fix"));
    usePr("ws");
    setPr("ws", pr("Fix"));
    expect(renders).toEqual([]);

    let done: (v: unknown) => void = () => {};
    prInfo.mockReturnValue(new Promise((r) => (done = r)));
    const reloading = reloadPr("ws");
    expect(usePr("ws")).toEqual(pr("Fix"));
    done({ status: "ok", data: pr("Fix it") });
    expect(await reloading).toEqual(pr("Fix it"));
    expect(renders.at(-1)).toEqual(pr("Fix it"));
  });
});
