import { describe, expect, it } from "vitest";
import type { GhPost } from "../bindings";
import { unanswered } from "./text";

const post = (author: string): GhPost => ({ author, body: author, createdAt: "", pending: false });

describe("unanswered", () => {
  it("splits at the viewer's last post", () => {
    const [before, after] = unanswered([post("bob"), post("me"), post("bob"), post("ann")], "me");
    expect(before.map((p) => p.author)).toEqual(["bob", "me"]);
    expect(after.map((p) => p.author)).toEqual(["bob", "ann"]);
  });

  it("shows the last post when the viewer had the last word", () => {
    const [before, after] = unanswered([post("bob"), post("me")], "me");
    expect([before.length, after.map((p) => p.author)]).toEqual([1, ["me"]]);
  });
});
