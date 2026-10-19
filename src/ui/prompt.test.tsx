// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import { confirmShown, PromptHost, promptKeeps, promptText } from "./prompt";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const tick = () => new Promise((r) => setTimeout(r, 0));

it("keeps a shown confirm up until a button or Escape answers it", async () => {
  const el = document.createElement("div");
  document.body.append(el);
  act(() => createRoot(el).render(<PromptHost />));
  let answer: boolean | undefined;
  act(() => {
    confirmShown("setup", "make", "Run", "Skip").then((a) => {
      answer = a;
    });
  });

  expect(await promptText("Rename")).toBeUndefined();
  expect(await confirmShown("other", "x", "Run")).toBe(false);
  const backdrop = el.querySelector(".prompt-backdrop") as HTMLElement;
  act(() => backdrop.dispatchEvent(new Event("pointerdown", { bubbles: true })));
  await tick();
  expect(answer).toBeUndefined();
  expect(el.querySelector("pre")?.textContent).toBe("make");

  const skip = el.querySelector("button") as HTMLButtonElement;
  act(() => skip.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })));
  await tick();
  expect(answer).toBe(false);
  expect(el.querySelector(".prompt")).toBeNull();
});

it("keeps focus off the backdrop, and swallows keys typed elsewhere while it's up", async () => {
  const el = document.createElement("div");
  document.body.append(el);
  act(() => createRoot(el).render(<PromptHost />));
  let answer: boolean | undefined;
  act(() => {
    confirmShown("setup", "make", "Run", "Skip").then((a) => {
      answer = a;
    });
  });
  const backdrop = el.querySelector(".prompt-backdrop") as HTMLElement;
  const down = new MouseEvent("mousedown", { bubbles: true, cancelable: true });
  act(() => backdrop.dispatchEvent(down));
  expect(down.defaultPrevented).toBe(true);

  const kept: boolean[] = [];
  const onKey = (e: KeyboardEvent) => kept.push(promptKeeps(e));
  window.addEventListener("keydown", onKey, true);
  const press = (key: string) => {
    const e = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true });
    act(() => document.body.dispatchEvent(e));
    return e;
  };
  expect(press("w").defaultPrevented).toBe(true);
  await tick();
  expect(answer).toBeUndefined();
  press("Escape");
  await tick();
  expect(answer).toBe(false);
  press("w");
  window.removeEventListener("keydown", onKey, true);
  expect(kept).toEqual([true, true, false]);
});
