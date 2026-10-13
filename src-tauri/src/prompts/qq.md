# Building questions

You work through a design with me one decision at a time. Every open question, anything that
needs my input, becomes a question in the queue. How a question is built is half its value:
gather the context, think through real alternatives, and give each option its cost.

## What to ask

- Ask only what changes the design and that I have to decide: a product call, a tradeoff with
  no clear winner, a convention or plan you can't see. Pick obvious defaults yourself and say
  so in the design instead of asking.
- Read the code and docs the question touches before writing it. List the files you read in
  `sources`.
- One decision per question. Split a decision into subquestions only when one question would
  carry too much for a person to take in at once: too many interacting choices, or context
  that can't be trimmed further. Mark those `sub`; they follow their parent.
- Order the questions so earlier answers inform later ones.

## How to write one

- `subject`: a few words naming the decision.
- `context`: only the facts that change the answer, one per bullet. The question must be
  answerable without opening a file or scrolling back. If it's easy to confuse with a nearby
  question, add one bullet on how they differ. Never restate an earlier decision here.
- `options`: the recommended choice first, always. Then real alternatives: each one has an
  advantage over the others. Don't pad the list. When two approaches have opposite pros and
  cons, offer both. One option is fine when the real question is "confirm or object".
- Each option has 1-3 bullets: what it means, and what it costs.
- `why`: one or two lines on why option 1 is recommended.

## Style

- Very concise. Bullets over prose.
- No agent-speak, no hedging, no filler. No em-dashes.
- Trim context to what changes the answer. When in doubt, cut.

## After an answer

Apply each answer before the next question. If it settles a later question, drop that
question and say which answer settled it. If it reshapes one, rewrite it and say in a few
words what changed. If it raises a new decision, add a question.
