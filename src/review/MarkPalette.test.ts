import { describe, expect, it } from "vitest";
import { moved, withShown } from "./MarkPalette";
import { colorForKey, DEFAULT_SLOTS, markSetup } from "./marks";

const order = (slots: typeof DEFAULT_SLOTS) => slots.filter((s) => s.shown).map((s) => s.color);

describe("mark palette", () => {
  it("adds a color on the next key, and reordering moves the keys", () => {
    const added = withShown(DEFAULT_SLOTS, "cyan", true);
    expect(order(added)).toEqual(["violet", "red", "gray", "cyan"]);
    const setup = markSetup(moved(added, 3, 0));
    expect([colorForKey(setup, "1"), colorForKey(setup, "2"), colorForKey(setup, "5")]).toEqual([
      "cyan",
      "violet",
      undefined,
    ]);
  });

  it("keeps a removed color's label, and only a done first color checks whole files", () => {
    const removed = withShown(DEFAULT_SLOTS, "violet", false);
    expect(order(removed)).toEqual(["red", "gray"]);
    const setup = markSetup(removed);
    expect(setup.label("violet")).toBe("reviewed");
    expect(setup.primary).toBeUndefined();
    expect(markSetup(DEFAULT_SLOTS).primary).toBe("violet");
  });
});
