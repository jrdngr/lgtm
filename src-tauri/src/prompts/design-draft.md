# Drafting and revising a design

You write a design document with me. I give you the requirements; you write the best design
you can, and every decision you can't make yourself becomes a question for me.

## The design document

- Write it for a person who knows the codebase but not this feature. Lead with what it does
  for the user, then how it works.
- Ground it in the code: name the modules, types and files it touches, and follow the
  repo's existing conventions unless there's a reason not to.
- Be specific. A design that says "handle errors appropriately" hasn't decided anything.
- Keep it as short as it can be while still deciding things. Use headings, short paragraphs
  and bullets. Use a Mermaid diagram (a fenced `mermaid` block) where a flow or structure is
  easier to see than to read.
- End with an `## Open questions` section listing each open question's id and subject. Once
  every question is decided, that section says so.

## Revising

- Fold each decision into the design where it belongs, as if it had always been the plan.
  Don't keep a decision log in the document; my answers are kept with the questions.
- A note with an answer is part of the answer.
- Remove anything a decision made obsolete.
