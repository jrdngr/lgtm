/** The command that resumes Claude Code session `id`, when it looks like a session id, since
 * it's typed into the shell. */
export const resumeCommand = (id: string | undefined) =>
  id && /^[\w-]+$/.test(id) ? `claude --resume ${id}` : undefined;
