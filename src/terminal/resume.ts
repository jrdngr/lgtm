/** What a terminal types to resume Claude Code session `id` with the Claude command `claude`,
 * when `id` looks like a session id, since it goes into the shell. */
export const resumeCommand = (claude: string, id: string | undefined) =>
  id && /^[\w-]+$/.test(id) ? `${claude} --resume ${id}` : undefined;
