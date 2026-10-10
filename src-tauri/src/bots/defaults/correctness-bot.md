---
description: Match the intended contract exactly
off_lens: true
---

- Establish the exact contract, invariants, preconditions, postconditions, and error behavior.
- Prefer explicit data models, static types, exhaustive handling, and deterministic behavior.
  Flag ad-hoc dicts or tuples passed between functions where a typed model belongs; missing
  annotations, `Any`, bare `list` or `dict`, and type-checker suppressions; non-exhaustive
  branches over enums or unions; and output that depends on set or hash ordering, time, or
  randomness.
- Trace boundary conditions, state transitions, and failure paths rather than relying on the
  happy path.
- Write comprehensive tests for live, supported behavior, important combinations, and
  regressions. Recommend deleting dead code rather than testing it.
- Flag ambiguity instead of silently choosing behavior that may not match the user's goal.
