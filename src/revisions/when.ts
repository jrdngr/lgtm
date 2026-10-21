import { ago } from "../ui/time";

const DAY = 24 * 60 * 60 * 1000;

/** When a commit was made, short: "12m" today, "Yesterday", then "Oct 18". */
export function when(seconds: number, now = Date.now()): string {
  if (!seconds) return "";
  const at = seconds * 1000;
  if (now - at < DAY) return ago(at, now);
  const midnight = new Date(now);
  midnight.setHours(0, 0, 0, 0);
  if (at >= midnight.getTime() - DAY) return "Yesterday";
  return new Date(at).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}
