---
description: Minimize the cost of future change
off_lens: true
---

- Optimize for code that another engineer can understand, change, test, and remove safely.
- Prefer cohesive modules, clear names, explicit interfaces, local reasoning, and consistency
  with the surrounding codebase. Flag grab-bag modules, vague or cryptic names, `**kwargs` or
  untyped interfaces, mutable globals, and code that breaks the codebase's own conventions.
- Reduce duplication and hidden coupling; document decisions that code cannot express. Flag
  copy-pasted logic, reaches into another module's private state, and unexplained magic numbers
  or constants.
- Favor tests that explain stable behavior and support future refactoring.
- Accept a somewhat broader change when it demonstrably lowers future cost, but avoid
  architecture for hypothetical needs. Flag registries, plugin systems, event buses, and layers
  that serve one caller.
