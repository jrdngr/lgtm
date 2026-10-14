import type { DockviewApi, IDockviewPanel } from "dockview-react";
import { expect, test, vi } from "vitest";
import { bottom } from "../workbench/bottom";
import { registerDock } from "../workbench/docks";
import { openSettings } from "./open";

/** A panel behind another tab in its pane. */
const panel = () =>
  ({
    api: { setActive: vi.fn(), updateParameters: vi.fn() },
    group: { activePanel: undefined },
  }) as unknown as IDockviewPanel;

const dock = (panels: Record<string, IDockviewPanel>) =>
  ({ getPanel: (id: string) => panels[id], addPanel: vi.fn() }) as unknown as DockviewApi;

test("Settings in the bottom dock is brought forward there, not opened again", () => {
  const settings = panel();
  const main = dock({});
  registerDock("ws", main);
  registerDock("ws", dock({ settings }), "bottom");
  bottom.set({ open: false });

  openSettings("ws", "prompts");

  expect(main.addPanel).not.toHaveBeenCalled();
  expect(bottom.get().open).toBe(true);
  expect(settings.api.setActive).toHaveBeenCalled();
  expect(settings.api.updateParameters).toHaveBeenCalledWith(
    expect.objectContaining({ section: "prompts" }),
  );
});
