import { afterEach, describe, expect, it } from "vitest";
import { keyPort } from "./keyPort";

const tick = () => new Promise((r) => setTimeout(r, 10));
const open: MessagePort[] = [];

/** A frame's side of an offer: the port it keeps, and the tokens the tab sends it. */
function frame() {
  const { port1, port2 } = new MessageChannel();
  open.push(port1, port2);
  const tokens: unknown[] = [];
  port1.onmessage = (e) => tokens.push(e.data);
  return { keep: port1, offered: port2, tokens };
}

afterEach(() => {
  for (const p of open.splice(0)) p.close();
});

describe("keyPort", () => {
  it("takes keys only over the first port offered, until its page says it's gone", async () => {
    const got: unknown[] = [];
    let n = 0;
    const keys = keyPort(
      (m) => got.push(m.key),
      () => `t${++n}`,
    );
    const real = frame();
    const forged = frame();
    keys.offer(real.offered);
    keys.offer(forged.offered);
    real.keep.postMessage({ key: "w" });
    forged.keep.postMessage({ key: "q" });
    await tick();
    expect(got).toEqual(["w"]);
    expect(real.tokens).toEqual(["t1"]);

    keys.gone("t2");
    keys.offer(frame().offered);
    real.keep.postMessage({ key: "e" });
    await tick();
    expect(got).toEqual(["w", "e"]);

    keys.gone("t1");
    const next = frame();
    keys.offer(next.offered);
    keys.offer(frame().offered);
    real.keep.postMessage({ key: "x" });
    next.keep.postMessage({ key: "n" });
    await tick();
    expect(got).toEqual(["w", "e", "n"]);
    expect(next.tokens).toEqual(["t2"]);
  });

  it("takes the first offer from a new frame", async () => {
    const got: unknown[] = [];
    const keys = keyPort((m) => got.push(m.key));
    keys.offer(frame().offered);
    keys.reset();
    const fresh = frame();
    keys.offer(fresh.offered);
    fresh.keep.postMessage({ key: "w" });
    fresh.keep.postMessage("not a key");
    await tick();
    expect(got).toEqual(["w"]);
  });
});
