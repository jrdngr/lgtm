// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { ArchDto } from "../bindings";
import type { CurrentReview } from "../review/current";

/** Each review's load, resolved by the test. */
const pending = new Map<number, (arch: ArchDto) => void>();
vi.mock("../ipc", () => ({
  commands: {
    reviewArch: (id: number) =>
      new Promise((done) => pending.set(id, (data) => done({ status: "ok", data }))),
  },
  unwrap: async (p: Promise<{ data: unknown }>) => (await p).data,
  errorText: String,
}));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const { useArch } = await import("./state");
const archOf = (name: string) => ({ components: [{ name }] }) as unknown as ArchDto;
const reviewOf = (id: number) => ({ id, files: [] }) as unknown as CurrentReview;

it("shows no architecture for a new review until its own loads", async () => {
  let shown: ArchDto | undefined;
  function Probe({ review }: { review: CurrentReview }) {
    shown = useArch(review).arch;
    return null;
  }
  const root = createRoot(document.createElement("div"));
  await act(() => root.render(<Probe review={reviewOf(1)} />));
  await act(async () => pending.get(1)?.(archOf("one")));
  expect(shown).toEqual(archOf("one"));

  await act(() => root.render(<Probe review={reviewOf(2)} />));
  expect(shown).toBeUndefined();
  await act(async () => pending.get(2)?.(archOf("two")));
  expect(shown).toEqual(archOf("two"));
});
