import { describe, expect, it } from "vitest";
import { activityOf, parseStatus } from "./programStatus";

describe("parseStatus", () => {
  it("reads the root record's state and the query, and leaves out the rest", () => {
    expect(parseStatus("state=working:app=claude-code:msg=SGk")).toBe("working");
    expect(parseStatus(" state = blocked : kind=permission")).toBe("blocked");
    expect(parseStatus("?")).toBe("query");
    expect(parseStatus("state=working:id=build/test")).toBeNull();
    expect(parseStatus("state=sleeping")).toBeNull();
    expect(parseStatus("app=cargo")).toBeNull();
  });

  it("shows blocked as waiting on input and clear as no agent", () => {
    expect(activityOf("blocked")).toBe("input");
    expect(activityOf("clear")).toBe("gone");
    expect(activityOf("done")).toBe("done");
  });
});
