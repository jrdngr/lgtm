import type { DockviewApi, MovePanelEvent } from "dockview-react";
import { describe, expect, it, vi } from "vitest";
import { moveTogether, pickedWith, togglePick } from "./tabPick";

describe("moveTogether", () => {
  it("brings the other picked tabs to the pane one of them was dragged to", () => {
    const from = { id: "g1" };
    const to = { id: "g2" };
    const panel = (id: string, group: object) => ({
      id,
      group,
      api: { moveTo: vi.fn(), setActive: vi.fn() },
    });
    const a = panel("a", to);
    const b = panel("b", from);
    const c = panel("c", from);
    let fire: (e: MovePanelEvent) => void = () => {};
    const api = {
      onDidMovePanel: (f: typeof fire) => {
        fire = f;
        return { dispose() {} };
      },
      getPanel: (id: string) => ({ a, b, c })[id as "a" | "b" | "c"],
    } as unknown as DockviewApi;
    moveTogether(api);

    togglePick("a", "b");
    expect(pickedWith("a").sort()).toEqual(["a", "b"]);
    fire({ panel: a, from, to } as unknown as MovePanelEvent);
    expect(b.api.moveTo).toHaveBeenCalledWith({ group: to, position: "center" });
    expect(c.api.moveTo).not.toHaveBeenCalled();
    expect(pickedWith("a")).toEqual([]);
  });
});
