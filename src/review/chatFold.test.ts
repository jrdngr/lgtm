import { describe, expect, it } from "vitest";
import type { Message } from "../bindings";
import { foldedCount } from "./chatFold";

const user = (text: string): Message => ({ author: "user", text });
const agent = (text: string): Message => ({ author: "agent", text });

describe("foldedCount", () => {
  it("folds everything before the latest exchange", () => {
    expect(foldedCount([user("a"), agent("b"), user("c"), agent("d")])).toBe(2);
  });

  it("folds the last answer while a new message waits for one", () => {
    expect(foldedCount([user("a"), agent("b"), user("c")])).toBe(2);
  });

  it("keeps the latest message when I haven't said anything", () => {
    expect(foldedCount([agent("a"), agent("b")])).toBe(1);
    expect(foldedCount([])).toBe(0);
  });
});
