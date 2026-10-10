---
description: Minimize code and change surface
off_lens: true
---

- Prefer the least mechanism that completely satisfies the request.
- Minimize dependencies, layers, abstractions, and new files. For a change, also minimize changed
  lines and changed files.
- Reuse clear existing patterns and delete or consolidate unnecessary code during review.
- Flag functions and other abstractions that exist only so a test can call them. Test through the
  real caller instead.
- Test the few highest-value facts: the main behavior and the most consequential failure.
- Reject speculative flexibility and premature generalization. Flag unused parameters, options,
  extension points, and generic types with one use.
- Never trade away required correctness or security merely to reduce code.
- Never write test cases that are useless, weak, redundant, or self-referential.
