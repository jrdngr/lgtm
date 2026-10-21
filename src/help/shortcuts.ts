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
      { keys: ["⌥⇥", "⌥⇧⇥"], action: "Next / previous pane" },
      { keys: ["⇧⌘↵"], action: "Maximize the focused pane, or restore the panes" },
      { keys: ["⌘B", "⌥⌘B"], action: "Show or hide the left / right panel" },
      { keys: ["⌘⇧K"], action: "Revisions (in an editor: delete the line)" },
      { keys: ["⌘⇧E"], action: "Changes" },
      { keys: ["⌘⇧R"], action: "Files" },
      { keys: ["⌘⇧M", "⌘⇧P"], action: "Comments" },
      { keys: ["⌘⇧O"], action: "Reports" },
      { keys: ["⌘⇧I"], action: "Q&A" },
      { keys: ["⌘⇧X"], action: "New drawing" },
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
      { keys: ["e", "E"], action: "Expand / shrink: line, function, hunk, file, review" },
      { keys: ["1–9"], action: "Mark with that color" },
      { keys: ["0"], action: "Clear the mark" },
      { keys: ["m"], action: "Mark menu (or right-click)" },
      { keys: ["⌘A"], action: "Select the whole review" },
      { keys: ["⌘Z", "⇧⌘Z"], action: "Undo / redo mark changes" },
      { keys: ["⌘C"], action: "Copy the selected text or rows" },
      { keys: ["c"], action: "Write a comment on the selection" },
      { keys: ["C"], action: "Draft a GitHub comment (PR workspaces)" },
      { keys: ["⌘>"], action: "Send the selection to the last terminal" },
      { keys: ["o"], action: "Open the file at the cursor in an editor" },
      { keys: ["x"], action: "Check or uncheck the file" },
      { keys: ["za"], action: "Collapse or expand the hunk or file" },
      { keys: ["g"], action: "Architecture" },
      { keys: ["]", "["], action: "Next / previous change" },
      { keys: ["}", "{"], action: "Next / previous file" },
      { keys: ["Esc"], action: "Back to the cursor row" },
    ],
  },
  {
    title: "Architecture",
    shortcuts: [
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
      { keys: ["↵", "e"], action: "Edit the commit" },
      { keys: ["r"], action: "Review the selection" },
      { keys: ["u", "U"], action: "Undo / redo the last edit" },
      { keys: ["Esc"], action: "Back to the review" },
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
