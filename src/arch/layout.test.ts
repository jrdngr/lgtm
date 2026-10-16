import { expect, test } from "vitest";
import { layoutArch, NODE_H, NODE_W } from "./layout";

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
