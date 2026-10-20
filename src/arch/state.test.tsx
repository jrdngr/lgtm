// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { ArchDto } from "../bindings";
import type { CurrentReview } from "../review/current";

/** Each review's load, resolved by the test. */
const pending = new Map<number, (arch: ArchDto) => void>();
/** The next check's result, resolved by the test. */
let checked: ((ok: boolean) => void) | undefined;
vi.mock("../ipc", () => ({
  commands: {
    reviewArch: (id: number) =>
      new Promise((done) => pending.set(id, (data) => done({ status: "ok", data }))),
    checkLook: () =>
      new Promise((done) => {
        checked = (ok) =>
          done(ok ? { status: "ok", data: null } : { status: "error", error: "no" });
      }),
  },
  unwrap: async (p: Promise<{ data: unknown }>) => (await p).data,
  errorText: String,
}));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const { checkLook, useArch } = await import("./state");
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

function probe() {
  const seen: { arch?: ArchDto } = {};
  function Probe({ review }: { review: CurrentReview }) {
    seen.arch = useArch(review).arch;
    return null;
  }
  return { seen, Probe };
}
const ofCommits = (id: number) =>
  ({ id, files: [], revset: "@-", head: "h1" }) as unknown as CurrentReview;
const withLook = () =>
  ({ components: [], look: [{ key: "a", checked: false }] }) as unknown as ArchDto;

it("shows the last architecture of the same commits while a new review of them loads", async () => {
  const { seen, Probe } = probe();
  const root = createRoot(document.createElement("div"));
  await act(() => root.render(<Probe review={ofCommits(10)} />));
  await act(async () => pending.get(10)?.(archOf("idle")));
  await act(() => root.render(<Probe review={ofCommits(11)} />));
  expect(seen.arch).toEqual(archOf("idle"));
  await act(async () => pending.get(11)?.(archOf("pane")));
  expect(seen.arch).toEqual(archOf("pane"));
});

it("checks a Look item at once and unchecks it when saving fails", async () => {
  const { seen, Probe } = probe();
  const review = { ...ofCommits(20), head: "h2" } as CurrentReview;
  const root = createRoot(document.createElement("div"));
  await act(() => root.render(<Probe review={review} />));
  await act(async () => pending.get(20)?.(withLook()));
  let saved: Promise<string | undefined> | undefined;
  await act(async () => {
    saved = checkLook(review, "a", true);
  });
  expect(seen.arch?.look[0].checked).toBe(true);
  await act(async () => checked?.(false));
  expect(await saved).toBe("no");
  expect(seen.arch?.look[0].checked).toBe(false);
});
