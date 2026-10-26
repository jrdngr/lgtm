// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { plain } from "./boot";

describe("plain", () => {
  it("turns byte arrays and maps into plain values, deeply", () => {
    const v = {
      rgba: new Uint8Array([1, 2]),
      m: new Map([["k", new Uint8Array([3])]]),
      n: [1, "x"],
    };
    expect(plain(v)).toEqual({ rgba: [1, 2], m: { k: [3] }, n: [1, "x"] });
  });

  it("keeps class instances, which serialize themselves", () => {
    class Channel {
      toJSON() {
        return "__CHANNEL__:1";
      }
    }
    const c = new Channel();
    expect((plain({ onEvent: c }) as { onEvent: unknown }).onEvent).toBe(c);
  });
});
