import { describe, expect, it } from "vitest";
import type { SymNodeDto } from "../bindings";
import { symbolsLayout } from "./symbolsLayout";

const sym = (id: number, path: string, component = "c"): SymNodeDto => ({
  id,
  name: `s${id}`,
  kind: "function",
  path,
  line: id,
  end: id,
  component,
  signature: "",
});

describe("symbolsLayout", () => {
  it("puts callers left of the frame and calls right, each ordered by where it links", () => {
    const placed = symbolsLayout({
      symbols: [sym(1, "c/a.ts"), sym(2, "c/a.ts")],
      callers: [sym(10, "x/u.ts", "x"), sym(11, "x/v.ts", "x")],
      calls: [sym(20, "y/w.ts", "y")],
      edges: [
        { from: 10, to: 2 },
        { from: 11, to: 1 },
        { from: 1, to: 20 },
      ],
    });
    const box = (id: number) => placed.syms.find((b) => b.sym.id === id);
    expect(box(10)?.side).toBe("caller");
    // The caller of the lower symbol sits lower.
    expect(box(11)?.y).toBeLessThan(box(10)?.y ?? 0);
    expect(box(10)?.x).toBeLessThan(placed.frame.x);
    expect(box(20)?.x).toBeGreaterThan(placed.frame.x + placed.frame.w);
    // Both symbols sit in their file's box, inside the frame.
    const file = placed.files[0];
    for (const id of [1, 2]) {
      const b = box(id);
      expect(b && b.y >= file.y && b.y + b.h <= file.y + file.h).toBe(true);
    }
    expect(file.y + file.h).toBeLessThanOrEqual(placed.frame.y + placed.frame.h);
  });
});
