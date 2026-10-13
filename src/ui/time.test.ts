import { describe, expect, it } from "vitest";
import { ago } from "./time";

const NOW = Date.parse("2026-10-13T12:00:00Z");
const secondsAgo = (seconds: number) => new Date(NOW - seconds * 1000).toISOString();

describe("ago", () => {
  it("rounds down into the largest unit that fits", () => {
    expect(ago(secondsAgo(10), NOW)).toBe("now");
    expect(ago(secondsAgo(90), NOW)).toBe("1m");
    expect(ago(secondsAgo(59 * 60), NOW)).toBe("59m");
    expect(ago(secondsAgo(3 * 3600 + 1800), NOW)).toBe("3h");
    expect(ago(secondsAgo(2 * 86400), NOW)).toBe("2d");
    expect(ago(secondsAgo(15 * 86400), NOW)).toBe("2w");
    expect(ago(secondsAgo(400 * 86400), NOW)).toBe("1y");
  });

  it("gives nothing for a date it can't read", () => {
    expect(ago("", NOW)).toBe("");
  });
});
