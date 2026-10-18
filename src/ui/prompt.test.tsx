// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import { confirmShown, PromptHost, promptText } from "./prompt";

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
