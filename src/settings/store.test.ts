import { expect, test, vi } from "vitest";

const settings = { claudeTerminal: "cc" };
vi.mock("../ipc", () => ({ commands: { getSettings: vi.fn(async () => settings) } }));
vi.mock("../bindings", () => ({ events: { settingsChanged: { listen: vi.fn() } } }));

test("loadedSettings resolves before and after the settings have loaded", async () => {
  const { loadedSettings } = await import("./store");
  expect(await loadedSettings()).toBe(settings);
  expect(await loadedSettings()).toBe(settings);
});
