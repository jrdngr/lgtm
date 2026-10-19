import { afterEach, describe, expect, it } from "vitest";
import { keyPort } from "./keyPort";

const tick = () => new Promise((r) => setTimeout(r, 10));
const open: MessagePort[] = [];

afterEach(() => {
  for (const p of open.splice(0)) p.close();
});

/** Connects, and gives back the frame's end of the port. */
function load(keys: ReturnType<typeof keyPort>) {
  let given: MessagePort | undefined;
  keys.connect((p) => {
    given = p;
  });
  if (!given) throw new Error("no port");
  open.push(given);
  return given;
}

describe("keyPort", () => {
  it("takes keys only over the port handed to the page loaded last", async () => {
    const got: unknown[] = [];
    const keys = keyPort((m) => got.push(m.key));
    const first = load(keys);
    first.postMessage({ key: "w" });
    first.postMessage("not a key");
    await tick();
    expect(got).toEqual(["w"]);

    const second = load(keys);
    first.postMessage({ key: "q" });
    second.postMessage({ key: "e" });
    await tick();
    expect(got).toEqual(["w", "e"]);

    keys.close();
    second.postMessage({ key: "x" });
    await tick();
    expect(got).toEqual(["w", "e"]);
  });
});
