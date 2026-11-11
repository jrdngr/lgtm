import { describe, expect, it } from "vitest";
import cases from "../../../crates/lgtm/src/edit/key_cases.json";
import { run } from "./spec.test-utils";

/** The TUI editor's key cases, which `cargo test` runs on its keys too. */
describe("modal keys", () => {
  for (const [group, list] of Object.entries(cases as Record<string, string[][]>)) {
    it(group, () => {
      for (const [start, spec, want] of list) {
        expect(run(start, spec), `${JSON.stringify(start)} with ${JSON.stringify(spec)}`).toBe(
          want,
        );
      }
    });
  }
});
