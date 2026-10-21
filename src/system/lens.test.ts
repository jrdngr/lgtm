import { describe, expect, it } from "vitest";
import { archOf, comp, edge } from "../arch/fixture.test-utils";
import type { LookDto, RowDto } from "../bindings";
import { focusLens, lensOf } from "./lens";

const row = (kind: RowDto["kind"]): RowDto => ({
  kind,
  oldLine: null,
  newLine: null,
  replaced: false,
  text: "",
  hl: null,
});

const look = (l: Partial<LookDto>): LookDto => ({
  key: "k",
  source: "rule",
  severity: "high",
  title: "",
  detail: null,
  component: null,
  edge: null,
  path: null,
  line: null,
  checked: false,
  ...l,
});

describe("lensOf", () => {
  it("sums each component's changed rows from the review files its files point at", () => {
    const arch = archOf([
      comp({
        id: "ui",
        status: "changed",
        files: [
          { path: "ui/a.ts", file: 0 },
          { path: "ui/b.ts", file: 1 },
          { path: "ui/c.ts", file: null },
        ],
      }),
    ]);
    const files = [
      { rows: [row("added"), row("added"), row("context")] },
      { rows: [row("removed"), row("added")] },
    ];
    const l = lensOf(arch, files);
    expect(l.delta.get("ui")).toEqual({ add: 3, del: 1 });
    expect(l.files.get("ui")?.map((f) => [f.path, f.add, f.del])).toEqual([
      ["ui/a.ts", 2, 0],
      ["ui/b.ts", 1, 1],
    ]);
  });

  it("keeps removed components and changed edges, and files open Look items by place", () => {
    const arch = archOf(
      [comp({ id: "old", status: "removed" }), comp({ id: "ui" })],
      [edge("ui", "old", { status: "removed" }), edge("ui", "db")],
      {
        look: [
          look({ key: "a", component: "ui" }),
          look({ key: "b", edge: ["ui", "db"] }),
          look({ key: "c", component: "ui", checked: true }),
        ],
      },
    );
    const l = lensOf(arch, []);
    expect(l.removed.map((k) => k.id)).toEqual(["old"]);
    expect(l.edges).toEqual([{ from: "ui", to: "old", status: "removed", imports: 0 }]);
    expect(l.look.get("ui")?.map((x) => x.key)).toEqual(["a", "b"]);
    expect(l.edgeLook.get("ui>db")?.map((x) => x.key)).toEqual(["b"]);
    expect(l.open).toBe(2);
  });
});

describe("focusLens", () => {
  it("leaves out components, wires and Look items the chunk's files don't touch", () => {
    const arch = archOf(
      [
        comp({
          id: "ui",
          status: "changed",
          files: [
            { path: "ui/a.ts", file: 0 },
            { path: "ui/b.ts", file: 1 },
          ],
        }),
        comp({ id: "db", status: "changed", files: [{ path: "db/q.ts", file: 2 }] }),
        comp({ id: "old", status: "removed" }),
      ],
      [edge("ui", "api", { status: "added" }), edge("db", "api", { status: "added" })],
      { look: [look({ key: "a", component: "ui" }), look({ key: "b", component: "db" })] },
    );
    const files = [{ rows: [row("added")] }, { rows: [row("removed")] }, { rows: [row("added")] }];
    const l = focusLens(lensOf(arch, files), ["ui/b.ts"]);
    expect(l.status.get("ui")).toBe("changed");
    expect(l.status.get("db")).toBe("unchanged");
    expect(l.removed).toEqual([]);
    expect(l.delta.get("ui")).toEqual({ add: 0, del: 1 });
    expect(l.files.get("ui")?.map((f) => f.path)).toEqual(["ui/b.ts"]);
    expect(l.edges.map((e) => e.from)).toEqual(["ui"]);
    expect(l.look.get("db")).toBeUndefined();
    expect(l.open).toBe(1);
  });
});
