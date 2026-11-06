import { isValidElement, type ReactElement, type ReactNode } from "react";
import { describe, expect, it } from "vitest";
import { paint } from "./paint";

type Piece = { text: string; marked: boolean; cls?: string };

function pieces(nodes: ReactNode[]): Piece[] {
  return nodes.map((n) => {
    const outer = n as ReactElement<{ children: ReactNode }>;
    const inner = outer.props.children;
    const span = isValidElement(inner)
      ? (inner as ReactElement<{ children: string; className: string }>)
      : undefined;
    return {
      text: span ? span.props.children : (inner as string),
      marked: outer.type === "mark",
      ...(span && { cls: span.props.className }),
    };
  });
}

describe("paint", () => {
  it("marks tinted chars counted past astral chars", () => {
    // 😀 is two UTF-16 units but one char, so chars 2..3 are "b".
    const out = pieces(paint("😀 b c", [], [[2, 3]]));
    expect(out).toEqual([
      { text: "😀 ", marked: false },
      { text: "b", marked: true },
      { text: " c", marked: false },
    ]);
  });

  it("splits highlight runs where a tint starts inside them", () => {
    const out = pieces(
      paint(
        "let x",
        [
          [0, 3],
          [null, 2],
        ],
        [[1, 4]],
      ),
    );
    expect(out.map((p) => [p.text, p.marked, !!p.cls])).toEqual([
      ["l", false, true],
      ["et", true, true],
      [" ", true, false],
      ["x", false, false],
    ]);
  });
});
