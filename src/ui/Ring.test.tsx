// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import { Ring } from "./Ring";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function render(done: number) {
  const el = document.createElement("div");
  act(() => createRoot(el).render(<Ring done={done} percent />));
  return el;
}

it("shows the check only once everything is done", () => {
  const almost = render(199 / 200);
  expect(almost.querySelector(".done")).toBeNull();
  expect(almost.textContent).toBe("99%");
  expect(render(1).querySelector(".done")).not.toBeNull();
});
