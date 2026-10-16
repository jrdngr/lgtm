/** Whether a terminal title says its agent is working: Claude Code leads it with a braille
 * spinner frame then, and with ✳ when idle or waiting on a prompt. */
export const isBusyTitle = (title: string) => /^[\u2801-\u28ff]/.test(title);

/** Whether a terminal title says Claude Code runs there, working or not. */
export const isAgentTitle = (title: string) => /^[\u2801-\u28ff\u2733]/.test(title);

/** The title without its leading spinner or idle mark, for the workspace's activity. */
export const titleLabel = (title: string) => title.replace(/^\S\s+/, "") || "Terminal";
