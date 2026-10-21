import { describe, expect, it } from "vitest";
import { graphCells } from "./graphCells";

const dirs = (c: { u: boolean; d: boolean; l: boolean; r: boolean }) =>
  `${c.u ? "u" : ""}${c.d ? "d" : ""}${c.l ? "l" : ""}${c.r ? "r" : ""}`;

describe("graphCells", () => {
  it("joins a node to the lines above and below it, and not past the graph's ends", () => {
    const cells = graphCells(["@  ", "│  ", "○  ", "│  ", "◆  ", "   "]);
    expect(dirs(cells[0][0])).toBe("d");
    expect(dirs(cells[2][0])).toBe("ud");
    expect(dirs(cells[4][0])).toBe("u");
    expect(cells[4][0].node).toBe(true);
  });

  it("reads a branch's join as corners and a tee", () => {
    const cells = graphCells(["│ ○  ", "├─╯  ", "○  "]);
    expect(dirs(cells[0][2])).toBe("d");
    expect(cells[1].map(dirs).slice(0, 3)).toEqual(["udr", "lr", "ul"]);
    expect(dirs(cells[2][0])).toBe("u");
  });
});
