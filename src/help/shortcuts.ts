/** Every keyboard shortcut, grouped by where it works, for Help › Keyboard Shortcuts. */

export interface Shortcut {
  /** Alternatives, shown side by side: `["⌘]", "⌘["]`. */
  keys: string[];
  action: string;
}

export interface ShortcutGroup {
  title: string;
  shortcuts: Shortcut[];
}

export const SHORTCUT_GROUPS: ShortcutGroup[] = [
  {
    title: "App",
    shortcuts: [
      { keys: ["⌘,"], action: "Settings" },
      { keys: ["⌘N"], action: "New window" },
      { keys: ["⌘O"], action: "Open a different repository" },
      { keys: ["⌘W"], action: "Close the active tab" },
      { keys: ["⇧⌘T"], action: "Reopen the last closed tab, again to go further back" },
      { keys: ["⇧⌘W"], action: "Close the window" },
      { keys: ["⌥Z"], action: "Word wrap" },
      { keys: ["⌘=", "⌘-", "⌘0"], action: "Zoom in, zoom out, actual size" },
    ],
  },
  {
    title: "Panes and panels",
    shortcuts: [
      { keys: ["⌃`"], action: "Show and focus the bottom panel, or hide it when focused" },
      { keys: ["⌃⇧`"], action: "New terminal in the bottom panel" },
      { keys: ["⌘T"], action: "New terminal tab in the focused grid" },
      { keys: ["⌘D"], action: "Split terminal right (in an editor: select the next match)" },
      { keys: ["⌘⇧D"], action: "Split terminal down" },
      { keys: ["⌘]", "⌘["], action: "Next / previous pane (in an editor: indent / outdent)" },
      { keys: ["⌃⇥", "⌃⇧⇥"], action: "Next / previous tab in the focused pane" },
      { keys: ["⌘-click"], action: "Pick tabs to drag together" },
      { keys: ["⌥⇥", "⌥⇧⇥"], action: "Next / previous pane" },
      { keys: ["⇧⌘↵"], action: "Maximize the focused pane, or restore the panes" },
      { keys: ["⌘B", "⌥⌘B"], action: "Show or hide the left / right panel" },
      { keys: ["⌘⇧K"], action: "Revisions (in an editor: delete the line)" },
      { keys: ["⌘⇧E"], action: "Changes" },
      { keys: ["⌘⇧R"], action: "Files" },
      { keys: ["⌘⇧M", "⌘⇧P"], action: "Comments" },
      { keys: ["⌘⇧O"], action: "Reports" },
      { keys: ["⌘⇧I"], action: "Q&A" },
      { keys: ["⌘⇧U"], action: "Guided Review" },
      { keys: ["⌘⇧X"], action: "New drawing" },
      { keys: ["⌘J"], action: "Revisions motions" },
      { keys: ["⌘F"], action: "Find in the editor, rendered page or review" },
      { keys: ["⇧⌘F"], action: "Search the workspace for the selection" },
      { keys: ["⌘P"], action: "Go to a file, @ a symbol, or :line" },
      { keys: ["⇧⌘J"], action: "Review only the working copy (@)" },
      { keys: ["⇧⌘N"], action: "New workspace" },
      { keys: ["⌥⌘N"], action: "Review a PR by URL or number, in its own workspace" },
      {
        keys: ["⇧⌘V"],
        action: "Markdown, SVG or HTML file: switch between editor and rendered view",
      },
    ],
  },
  {
    title: "Editor or rendered markdown",
    shortcuts: [
      { keys: ["⌘>"], action: "Send the selection's path and lines to the last terminal" },
    ],
  },
  {
    title: "Symbols, in the editor or on a selection",
    shortcuts: [
      { keys: ["F12", "⌘-click"], action: "Go to definition" },
      { keys: ["⇧F12"], action: "Find all references" },
      { keys: ["⌥F12"], action: "Symbol information" },
    ],
  },
  {
    title: "Search tab",
    shortcuts: [
      { keys: ["↑", "↓"], action: "Move between results" },
      { keys: ["↵"], action: "Open the result" },
      { keys: ["⌥↵"], action: "Send the results to the terminal" },
      { keys: ["⌘↵"], action: "Replace all" },
    ],
  },
  {
    title: "HTML file, rendered",
    shortcuts: [
      { keys: ["⌘←", "⌘→"], action: "Back / forward" },
      { keys: ["⌘R"], action: "Reload" },
      { keys: ["⌘F"], action: "Find in the page" },
      { keys: ["⌥⌘=", "⌥⌘-", "⌥⌘0"], action: "Zoom the page in, out, actual size" },
      { keys: ["⌥⌘9"], action: "Fit the page to the tab's width" },
    ],
  },
  {
    title: "Image or rendered SVG",
    shortcuts: [
      { keys: ["⌥⌘=", "⌥⌘-", "⌥⌘0"], action: "Zoom in, out, actual size" },
      { keys: ["⌥⌘9"], action: "Fit the image to the tab" },
    ],
  },
  {
    title: "Review",
    shortcuts: [
      { keys: ["j", "k"], action: "Move the cursor" },
      { keys: ["J", "K"], action: "Extend the selection (or v, then j / k)" },
      { keys: ["g", "G"], action: "Top / bottom" },
      { keys: ["⌃D", "⌃U"], action: "Half a page down / up" },
      { keys: ["h", "l"], action: "Scroll sideways" },
      { keys: ["n", "N"], action: "Next / previous file (with Find open: next / previous match)" },
      { keys: ["m", "M"], action: "Next / previous syntax item" },
      { keys: ["]", "["], action: "Next / previous comment" },
      { keys: ["e", "E"], action: "Expand / shrink: line, function, hunk, file, review" },
      { keys: ["r"], action: "Check or uncheck the file" },
      { keys: ["f"], action: "Flag the file" },
      { keys: ["R", "F"], action: "Mark the hunk reviewed / flagged" },
      { keys: ["1–9"], action: "Mark with that color" },
      { keys: ["0"], action: "Clear the mark" },
      { keys: ["u", "U"], action: "Undo / redo mark changes (or ⌘Z, ⇧⌘Z)" },
      { keys: ["c"], action: "Comment for the agent" },
      { keys: ["↵"], action: "Edit the comment, or fold the file on its header" },
      { keys: ["d"], action: "Delete the comment" },
      { keys: ["C"], action: "Resolve or reopen the comment" },
      { keys: ["/"], action: "Find in the review" },
      { keys: ["z"], action: "Set reviewed files apart in Changes" },
      { keys: ["P"], action: "Show or hide PR comments" },
      { keys: ["p"], action: "Pull PR comments" },
      { keys: ["t"], action: "Side by side" },
      { keys: ["⌘A"], action: "Select the whole review" },
      { keys: ["⌘C"], action: "Copy the selected text or rows" },
      { keys: ["⌘>"], action: "Send the selection to the last terminal" },
      { keys: ["Esc"], action: "Back to the cursor row" },
      { keys: ["space f"], action: "Go to a file" },
      { keys: ["space b"], action: "Show or hide the right panel" },
      { keys: ["space /"], action: "Search the workspace" },
      { keys: ["space w"], action: "Word wrap" },
      { keys: ["space r"], action: "Open the file at the cursor in an editor" },
      { keys: ["space c"], action: "Draft a GitHub comment (PR workspaces)" },
      { keys: ["space m"], action: "Mark menu (or right-click)" },
      { keys: ["space a"], action: "Architecture" },
    ],
  },
  {
    title: "Guided Review",
    shortcuts: [
      { keys: ["a"], action: "Approve the chunk and go to the next" },
      {
        keys: ["c"],
        action: "Change: send the chunk and a note to the agent, then go to the next",
      },
      { keys: ["?"], action: "Ask the agent about the chunk" },
      { keys: ["←", "→"], action: "Previous / next step" },
      { keys: ["j", "k"], action: "Pick the next / previous line" },
      { keys: ["J", "K"], action: "Extend the picked lines" },
      { keys: ["⇧C"], action: "With lines picked: draft for the PR" },
      { keys: ["1–9", "0"], action: "Mark the picked lines, or clear their mark" },
      { keys: ["⌘>"], action: "Send the picked lines to the send target" },
      { keys: ["⌘C"], action: "Copy the picked lines" },
      { keys: ["o"], action: "Open the line in the editor" },
      { keys: ["⌘Z", "⇧⌘Z"], action: "Undo / redo marks" },
      { keys: ["p"], action: "Promote excluded code, then 1–3 for its risk" },
    ],
  },
  {
    title: "Architecture",
    shortcuts: [
      { keys: ["click", "⌘-click"], action: "Drill down / select" },
      { keys: ["/"], action: "Find a component, file or symbol" },
      { keys: ["⌘>"], action: "Ask the send target about the selection" },
      { keys: ["←", "→"], action: "Previous / next tour step" },
      { keys: ["Esc"], action: "End the trace or tour, or clear the selection" },
      { keys: ["⌫"], action: "Up a level" },
    ],
  },
  {
    title: "Revisions",
    shortcuts: [
      { keys: ["j", "k"], action: "Move" },
      { keys: ["J", "K"], action: "Extend a range" },
      { keys: ["↵"], action: "Edit the commit" },
      { keys: ["r"], action: "Review the selection" },
      { keys: ["Esc"], action: "Back to the review" },
    ],
  },
  {
    title: "Motions (⌘J)",
    shortcuts: [
      { keys: ["na", "nb"], action: "New commit after / before the cursor" },
      { keys: ["d"], action: "Describe the cursor's commit" },
      { keys: ["s"], action: "Squash the selection into its parent" },
      { keys: ["ba"], action: "Advance the closest bookmark to the working copy" },
      { keys: ["bc"], action: "Create a bookmark at the cursor" },
      { keys: ["bs"], action: "Set a nearby bookmark to the cursor" },
      { keys: ["u", "U"], action: "Undo / redo" },
      { keys: ["gf", "gp"], action: "Fetch / push" },
      { keys: ["gr"], action: "Rebase the cursor's branch onto main@origin" },
    ],
  },
  {
    title: "PR comments",
    shortcuts: [
      { keys: ["e"], action: "Edit the reply (in a focused comment)" },
      { keys: ["y"], action: "Mark the reply final (in a focused comment)" },
      { keys: ["q"], action: "Quote the selected text of a comment on the PR" },
      { keys: ["t"], action: "Triage new threads (in the PR panel)" },
      { keys: ["l"], action: "Link the send target's agent to the PR review (in the PR panel)" },
      { keys: ["⌘>"], action: "Talk about the thread with the linked agent" },
    ],
  },
  {
    title: "Q&A questions",
    shortcuts: [
      { keys: ["1–9"], action: "Select an option" },
      { keys: ["↑", "↓"], action: "Previous / next option (or k / j)" },
      { keys: ["↵"], action: "Decide the selected option" },
      { keys: ["n"], action: "Add a note to the option" },
      { keys: ["o"], action: "Answer with none of the options" },
      { keys: ["d"], action: "Discuss with the agent" },
      { keys: ["s"], action: "Defer it and what depends on it" },
      { keys: ["u"], action: "Undo the last decision or defer" },
      { keys: ["r"], action: "Change a decided answer" },
      { keys: ["p"], action: "Show a reopened question's previous text" },
      { keys: ["]", "["], action: "Look at the next / previous question" },
      { keys: ["esc"], action: "Back to the question to answer" },
    ],
  },
];

/** The groups with only the shortcuts whose keys or action contain `query`, ignoring case. */
export function filterShortcuts(groups: ShortcutGroup[], query: string): ShortcutGroup[] {
  const q = query.trim().toLowerCase();
  if (!q) return groups;
  return groups
    .map((g) => ({
      ...g,
      shortcuts: g.shortcuts.filter(
        (s) => s.action.toLowerCase().includes(q) || s.keys.some((k) => k.toLowerCase() === q),
      ),
    }))
    .filter((g) => g.shortcuts.length > 0);
}
