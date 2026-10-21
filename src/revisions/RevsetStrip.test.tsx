// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { publishLogRevset } from "./logRevset";
import { RevsetStrip } from "./RevsetStrip";

const sent = vi.hoisted(() => [] as string[]);
vi.mock("@tauri-apps/api/webviewWindow", () => ({ getCurrentWebviewWindow: () => ({}) }));
vi.mock("../bindings", async (actual) => ({
  ...(await actual<object>()),
  commands: {
    setLogRevset: (_workspace: unknown, revset: string) => {
      sent.push(revset);
      return Promise.resolve(
        revset.endsWith("(")
          ? { status: "error", error: "Failed to parse revset" }
          : { status: "ok", data: null },
      );
    },
  },
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let root: ReturnType<typeof createRoot>;
beforeEach(() => {
  sent.length = 0;
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

function show(revset: string | null, editing: boolean, error: string | null = null) {
  act(() => {
    publishLogRevset("w", { revset, error });
    root.render(<RevsetStrip workspace="w" editing={editing} setEditing={() => {}} />);
  });
}

async function enter(text: string) {
  const input = host.querySelector("input") as HTMLInputElement;
  input.value = text;
  await act(async () => {
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  });
}

describe("RevsetStrip", () => {
  it("is hidden while the log shows jj's default and nobody is editing", () => {
    show(null, false);
    expect(host.innerHTML).toBe("");
  });

  it("applies the revset on Enter and keeps jj's refusal under the input", async () => {
    show(null, true);
    await enter("mine() & author(");
    expect(sent).toEqual(["mine() & author("]);
    expect(host.querySelector(".revset-error")?.textContent).toBe("Failed to parse revset");
    await enter("mine()");
    expect(host.querySelector(".revset-error")).toBeNull();
  });

  it("shows a custom revset when closed, with its fallback error, and resets to the default", async () => {
    show("gone", false, "Revision `gone` doesn't exist");
    expect(host.querySelector(".revset-text")?.textContent).toBe("gone");
    expect(host.querySelector(".revset-error")?.textContent).toContain("gone");
    await act(async () => {
      (host.querySelector('[aria-label="Reset"]') as HTMLButtonElement).click();
    });
    expect(sent).toEqual([""]);
  });
});
