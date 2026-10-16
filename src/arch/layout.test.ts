import { expect, test } from "vitest";
import { layoutArch, legs, NODE_H, NODE_W } from "./layout";

test("components sit inside their container, users left of what they use", async () => {
  const placed = await layoutArch({
    groups: [{ id: "c:src", children: ["review", "github"] }],
    nodes: ["review", "github", "cli"],
    edges: [
      { key: "cli>review", from: "cli", to: "review" },
      { key: "review>github", from: "review", to: "github" },
    ],
  });
  const group = placed.groups.get("c:src");
  const review = placed.nodes.get("review");
  const github = placed.nodes.get("github");
  const cli = placed.nodes.get("cli");
  if (!group || !review || !github || !cli) throw new Error("not placed");
  for (const n of [review, github]) {
    expect(n.x).toBeGreaterThanOrEqual(group.x);
    expect(n.y).toBeGreaterThanOrEqual(group.y);
    expect(n.x + NODE_W).toBeLessThanOrEqual(group.x + group.width);
    expect(n.y + NODE_H).toBeLessThanOrEqual(group.y + group.height);
  }
  expect(cli.x + NODE_W).toBeLessThanOrEqual(group.x);
  expect(review.x + NODE_W).toBeLessThanOrEqual(github.x);
  expect(placed.routes.get("review>github")?.length).toBeGreaterThanOrEqual(2);
});

test("edges between two containers share one trunk between them", async () => {
  const placed = await layoutArch({
    groups: [
      { id: "c:app", children: ["ui", "cli"] },
      { id: "c:lib", children: ["db", "net"] },
    ],
    nodes: ["ui", "cli", "db", "net"],
    edges: [
      { key: "ui>db", from: "ui", to: "db" },
      { key: "cli>net", from: "cli", to: "net" },
    ],
  });
  const app = placed.groups.get("c:app");
  const lib = placed.groups.get("c:lib");
  const a = placed.routes.get("ui>db");
  const b = placed.routes.get("cli>net");
  if (!app || !lib || !a || !b) throw new Error("not placed");
  // The stretch between the containers' facing sides is the same for both edges.
  const between = (r: typeof a) =>
    r.filter((p) => p.x >= app.x + app.width && p.x <= lib.x).map((p) => `${p.x},${p.y}`);
  expect(between(a).length).toBeGreaterThan(0);
  expect(between(a)).toEqual(between(b));
});

test("an edge's legs join into one path from its start to its end", () => {
  const { legsOf, ports } = legs({
    groups: [{ id: "g", children: ["a"] }],
    nodes: ["a", "x"],
    edges: [{ key: "a>x", from: "a", to: "x" }],
  });
  expect(legsOf.get("a>x")).toEqual(["a>>out:g>x", "out:g>x>>x"]);
  expect(ports.get("out:g>x")).toEqual({ group: "g", side: "EAST" });
});
