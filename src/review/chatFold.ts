import type { Message } from "../bindings";

/** How many of a chat's messages fold away: all before the latest message of mine, so the
 * latest exchange shows, or just my message while the agent answers it. */
export function foldedCount(messages: Message[]): number {
  for (let i = messages.length - 1; i >= 0; i--) if (messages[i].author === "user") return i;
  return Math.max(0, messages.length - 1);
}
