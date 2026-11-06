import { describe, expect, it } from "vitest";
import {
  applyEdit,
  type MdEdit,
  mdEnter,
  mdRenumber,
  mdSmart,
  mdToggleCheck,
  toggleLine,
} from "./mdEdit";

/** Runs `f` on `text` with the cursor at `‸`, and gives the result with `‸` at the cursor. */
function run(f: (text: string, at: number) => MdEdit | undefined, marked: string) {
  const at = marked.indexOf("‸");
  const text = marked.replace("‸", "");
  const edit = f(text, at);
  if (!edit) return undefined;
  const out = applyEdit(text, edit);
  return `${out.slice(0, edit.cursor)}‸${out.slice(edit.cursor)}`;
}

describe("mdEnter", () => {
  it("goes on with the same bullet, the next number or letter, and an empty box", () => {
    const cases = [
      ["- a‸", "- a\n- ‸"],
      ["  * a‸", "  * a\n  * ‸"],
      ["+   a‸", "+   a\n+   ‸"],
      ["9. a‸", "9. a\n10. ‸"],
      ["1) a‸", "1) a\n2) ‸"],
      ["a. x‸", "a. x\nb. ‸"],
      ["C) x‸", "C) x\nD) ‸"],
      ["- [x] done‸", "- [x] done\n- [ ] ‸"],
      ["1. [ ] a‸", "1. [ ] a\n2. [ ] ‸"],
      ["- ab‸cd", "- ab\n- ‸cd"],
      ["> quote‸", "> quote\n> ‸"],
      ["> > deep‸", "> > deep\n> > ‸"],
    ];
    for (const [from, to] of cases) expect(run(mdEnter, from), from).toBe(to);
  });

  it("ends an empty item or quote, and does nothing elsewhere", () => {
    expect(run(mdEnter, "- a\n- ‸")).toBe("- a\n‸");
    expect(run(mdEnter, "- a\n- [ ] ‸")).toBe("- a\n‸");
    expect(run(mdEnter, "> a\n> ‸")).toBe("> a\n‸");
    expect(run(mdEnter, "plain‸")).toBeUndefined();
    expect(run(mdEnter, "‸- a")).toBeUndefined();
    expect(run(mdEnter, "---‸")).toBeUndefined();
    expect(run(mdEnter, "e.g.‸")).toBeUndefined();
  });

  it("renumbers the items after it in an ordered list but not nested ones", () => {
    const from = "1. a‸\n2. b\n   1. inner\n3. c\n\npara\n4. other";
    const to = "1. a\n2. ‸\n3. b\n   1. inner\n4. c\n\npara\n4. other";
    expect(run(mdEnter, from)).toBe(to);
  });

  it("adds a header's delimiter row, and starts a new row after a row", () => {
    expect(run(mdEnter, "| Name      | Age |‸")).toBe(
      "| Name      | Age |\n| --------- | --- |\n| ‸",
    );
    expect(run(mdEnter, "| a |\n| --- |\n| b |‸")).toBe("| a |\n| --- |\n| b |\n| ‸");
  });
});

describe("mdRenumber", () => {
  it("fixes a list from its first item whichever line the cursor is on", () => {
    expect(run(mdRenumber, "x\n\n3. a\n5. b\n   more\n‸9. c\n- d")).toBe(
      "x\n\n3. a\n4. b\n   more\n‸5. c\n- d",
    );
    expect(run(mdRenumber, "1. a\n2. ‸b")).toBeUndefined();
    expect(run(mdRenumber, "a\n- ‸b\n- c")).toBeUndefined();
  });
});

describe("mdToggleCheck", () => {
  it("flips a box and gives a plain item one", () => {
    expect(run(mdToggleCheck, "- [ ] a‸")).toBe("- [x] a‸");
    expect(run(mdToggleCheck, "  1. [X] ‸a")).toBe("  1. [ ] ‸a");
    expect(run(mdToggleCheck, "- a‸")).toBe("- [ ] a‸");
    expect(run(mdToggleCheck, "‸- a")).toBe("‸- [ ] a");
    expect(run(mdToggleCheck, "plain‸")).toBeUndefined();
  });

  it("toggles a rendered checkbox's line but leaves lines without one", () => {
    expect(toggleLine("x\n- [ ] a\n", 2)).toBe("x\n- [x] a\n");
    expect(toggleLine("x\n- a\n", 2)).toBeUndefined();
  });
});

describe("mdSmart footnotes", () => {
  it("writes a reference's note at the bottom, or goes to the one there", () => {
    expect(run(mdSmart, "Body[^a]‸ text.\n")).toBe("Body[^a] text.\n\n[^a]: ‸\n");
    expect(run(mdSmart, "B[^‸a] c[^b].\n\n[^b]: one\n")).toBe(
      "B[^a] c[^b].\n\n[^b]: one\n[^a]: ‸\n",
    );
    expect(run(mdSmart, "B[^a]‸.\n\n[^a]: note\n")).toBe("B[^a].\n\n[^a]: ‸note\n");
    expect(run(mdSmart, "B[^a].‸")).toBeUndefined();
  });

  it("numbers a new footnote after a bare caret", () => {
    expect(run(mdSmart, "A[^1] b[^‸\n\n[^1]: one\n")).toBe("A[^1] b[^2]\n\n[^1]: one\n[^2]: ‸\n");
    expect(run(mdSmart, "x[^‸")).toBe("x[^1]\n\n[^1]: ‸");
  });

  it("goes from a note back to its reference", () => {
    expect(run(mdSmart, "See[^n] and [^m].\n\n[^m]: em‸\n")).toBe(
      "See[^n] and [^m]‸.\n\n[^m]: em\n",
    );
  });
});

describe("mdSmart tables", () => {
  it("starts a column in a header without a delimiter", () => {
    expect(run(mdSmart, "| Name      ‸")).toBe("| Name      | ‸");
    expect(run(mdSmart, "| Name‸")).toBe("| Name | ‸");
  });

  it("pads the cell to its column and moves to the next, or a new row from the last", () => {
    expect(run(mdSmart, "| Name      | Age |\n| --- | :-: |\n| Bob‸")).toBe(
      "| Name      | Age |\n| --------- | :-: |\n| Bob       | ‸",
    );
    expect(run(mdSmart, "| Name      | Age |\n| --- | --- |\n| Bob       | 4‸")).toBe(
      "| Name      | Age |\n| --------- | --- |\n| Bob       | 4   |\n| ‸",
    );
  });

  it("widens a column whose cell outgrows the header", () => {
    expect(run(mdSmart, "| A   | B |\n|---|--:|\n| longer‸")).toBe(
      "| A      | B   |\n| ------ | --: |\n| longer | ‸",
    );
  });
});
