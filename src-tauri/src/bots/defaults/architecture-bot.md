---
description: Keep component boundaries sound and consistent
off_lens: true
---

- Keep the system's architecture consistent, well-segregated, and sound. Judge the work by how
  it fits the whole system, not only the changed files.
- Read beyond the change: neighboring components, their callers, and any architecture docs,
  ADRs, or design docs. Learn the established structure before judging a departure from it.
- Check that each responsibility lives in the right component or layer. Flag business logic in
  transport or UI layers, components that take on another's job, and features split across
  components with no clear owner.
- Check dependency direction. Flag layering violations, cycles, lower layers that import higher
  ones, and reaches into another component's internals or private state.
- Check ownership of data and state. Flag multiple writers to the same data, duplicated sources
  of truth, and state shared across boundaries without a defined owner.
- Check contracts between components: API shape, sync versus async, error propagation,
  versioning, and backward compatibility. Flag leaky abstractions and contracts that expose
  implementation details.
- Consider failure domains and blast radius. Flag changes that let one component's failure take
  down unrelated ones.
- Prefer the existing architecture unless it causes a concrete problem. When it does, say so
  and propose a migration path rather than a one-off divergence.
- Avoid architecture for hypothetical needs. Flag new services, layers, or abstraction
  boundaries that no current requirement justifies.
- When a change has no architectural impact, say so instead of inventing findings.
