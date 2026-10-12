Write a walkthrough of this change for the human reviewing it: notes pinned to the code that tell them where to look, in what order, and what to check. The reviewer reads the diff anyway, so don't narrate it. Each note should tell them something they'd otherwise have to work out: why the code is the way it is, what could go wrong, how the pieces connect, what isn't there. Read the surrounding code when the diff isn't enough.

Return the notes in the order the reviewer should read them. Start with a context note on the change's entry point saying in two or three sentences what the change does and how it's organized. Then follow the change the way it runs, not file by file. Usually 5 to 20 notes; fewer for a small change. Skip anything routine.

Each note has a repository-relative path, the first and last lines on the new side of the diff (the old side for a deleted file), a category, a title, and a body.

- title: one line, under 80 characters, that makes sense on its own in a list.
- body: one to four sentences of Markdown. Concrete: name the variable, the case, the caller.
- Keep ranges tight: a function or a few lines, not a whole file.

Categories:
- important: needs a careful look. Subtle logic, edge cases, concurrency, error handling, security boundaries, data and schema changes, public interfaces, behavior tests don't cover. Say what to check. Give a level: high (likely wrong, or a mistake would do serious damage), medium (easy to get wrong), or low (routine, but worth knowing).
- flow: a step in a path through several places that's hard to follow from the diff, like a request going through handlers or a value threaded through layers. Give each flow a short name and number its steps from 1, so the reviewer can step through it alone. Only for flows of three or more steps; one change can have several flows.
- context: intent or background the diff doesn't show, like why an approach was chosen or what a removed piece used to do.
- question: something you couldn't settle from the code and the reviewer or author should, like a behavior change that may or may not be intended.
