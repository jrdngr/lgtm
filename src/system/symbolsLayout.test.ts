import { describe, expect, it } from "vitest";
import type { SymbolsDto, SymNodeDto } from "../bindings";
import { symbolsLayout } from "./symbolsLayout";
import { focusLayout, hairlines, linksOf } from "./symbolsLinks";

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

const dto: SymbolsDto = {
  symbols: [sym(1, "c/a.ts"), sym(2, "c/a.ts"), sym(3, "c/b.ts")],
  callers: [sym(10, "x/u.ts", "x"), sym(11, "x/v.ts", "x"), sym(12, "z/w.ts", "z")],
  calls: [sym(20, "y/w.ts", "y")],
  edges: [
    { from: 10, to: 2 },
    { from: 11, to: 1 },
    { from: 12, to: 1 },
    { from: 1, to: 20 },
    { from: 3, to: 1 },
    { from: 2, to: 1 },
  ],
};

describe("symbolsLayout", () => {
  const placed = symbolsLayout(dto);
  const chip = (key: string) => placed.chips.find((c) => c.key === key);

  it("groups outside symbols into one chip per component and side", () => {
    expect(placed.chips.map((c) => c.key).sort()).toEqual(["call:y", "caller:x", "caller:z"]);
    expect(chip("caller:x")?.ids).toEqual([10, 11]);
    expect(chip("caller:x")?.x).toBeLessThan(placed.frame.x);
    expect(chip("call:y")?.x).toBeGreaterThan(placed.frame.x + placed.frame.w);
  });

  it("puts each symbol in its file's box, inside the frame", () => {
    for (const b of placed.syms.filter((s) => s.side === "in")) {
      const file = placed.files.find((f) => f.path === b.sym.path);
      expect(file && b.y >= file.y && b.y + b.h <= file.y + file.h).toBe(true);
      expect(file && file.y + file.h <= placed.frame.y + placed.frame.h).toBe(true);
    }
  });

  it("draws one line per pair of files or chips that call each other", () => {
    // a.ts with x, z, y and b.ts; calls inside a.ts draw nothing.
    expect(hairlines(dto, placed)).toHaveLength(4);
  });
});

describe("linksOf", () => {
  const placed = symbolsLayout(dto);

  it("draws an outside end as its chip at rest, and as itself in a graph", () => {
    expect(linksOf(dto, placed, "s1", true)).toContainEqual(["caller:x", "s1"]);
    expect(linksOf(dto, placed, "s1", false)).toContainEqual(["s11", "s1"]);
  });

  it("gives a chip its component's links", () => {
    expect(linksOf(dto, placed, "caller:x", false)).toEqual([
      ["caller:x", "s2"],
      ["caller:x", "s1"],
    ]);
  });
});

describe("focusLayout", () => {
  it("puts callers left, the node in the middle and calls right", () => {
    const f = focusLayout(dto, symbolsLayout(dto), "s1");
    const x = (k: string) => f.pos.get(k)?.x;
    for (const k of ["s11", "s12", "s3", "s2"]) expect(x(k)).toBe(0);
    expect(x("s1")).toBeGreaterThan(0);
    expect(x("s20")).toBeGreaterThan(x("s1") ?? 0);
    expect(f.pos.size).toBe(6);
    expect(f.links).toHaveLength(5);
  });
});
