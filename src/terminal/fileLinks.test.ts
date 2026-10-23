import { describe, expect, it } from "vitest";
import { pathsIn } from "./fileLinks";

const paths = (text: string) => pathsIn(text).map(({ path, line }) => ({ path, line }));

describe("pathsIn", () => {
  it('takes the line after a colon or #L, and from a "line N of" before the path', () => {
    expect(
      paths("see src/a.ts:12:3, lib/b.rs#L7 and line 35 of services/auth/statements.py:"),
    ).toEqual([
      { path: "src/a.ts", line: 12 },
      { path: "lib/b.rs", line: 7 },
      { path: "services/auth/statements.py", line: 35 },
    ]);
  });

  it("spans the path and its line, leaving out a sentence's last dot", () => {
    const text = "Edit README.md. Then foo/bar.ts:9";
    expect(pathsIn(text).map((p) => text.slice(p.from, p.to))).toEqual([
      "README.md",
      "foo/bar.ts:9",
    ]);
  });

  it("skips plain words and the hosts and paths of web addresses", () => {
    expect(paths("run it at https://github.com/a/b.ts now")).toEqual([]);
  });
});
