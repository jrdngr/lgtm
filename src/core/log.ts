import { commands, type LogLevel } from "../bindings";

const RANK: Record<LogLevel, number> = { error: 0, warn: 1, info: 2, debug: 3, trace: 4 };

/** The most detailed level the app keeps for the UI. INFO until it says. */
let kept: LogLevel = "info";

/** Asks the app which UI levels it keeps, so the rest aren't sent. */
export function loadLogLevel() {
  Promise.resolve()
    .then(() => commands.logLevel())
    .then(
      (l) => {
        kept = l;
      },
      () => {},
    );
}

/** One value of a log line as text: an error with its stack, anything else as JSON. */
export function logText(v: unknown): string {
  if (v instanceof Error) return v.stack ?? `${v.name}: ${v.message}`;
  if (typeof v === "string") return v;
  try {
    return JSON.stringify(v) ?? String(v);
  } catch {
    return String(v);
  }
}

type Log = (...parts: unknown[]) => void;

/** Logs for `module` to the app's output, with target `ui`, as `RUST_LOG` filters it. */
export function logger(module: string): Record<LogLevel, Log> {
  const at =
    (level: LogLevel): Log =>
    (...parts) => {
      if (RANK[level] > RANK[kept]) return;
      // A log line that can't reach the app has nowhere else to go.
      const text = parts.map(logText).join(" ");
      Promise.resolve()
        .then(() => commands.log(level, module, text))
        .catch(() => {});
    };
  return {
    error: at("error"),
    warn: at("warn"),
    info: at("info"),
    debug: at("debug"),
    trace: at("trace"),
  };
}

const uncaught = logger("uncaught");

/** WebKit reports a ResizeObserver that resized within its own callback as an error, though
 * nothing failed: the rest of its notifications arrive next frame. */
const RESIZE_LOOP = "ResizeObserver loop";

/** Sends uncaught errors and unhandled rejections to the app's log. */
export function installErrorLogging() {
  window.addEventListener("error", (e) => {
    if (!e.error && e.message?.startsWith(RESIZE_LOOP)) return;
    uncaught.error(e.error ?? e.message);
  });
  window.addEventListener("unhandledrejection", (e) => uncaught.error(e.reason));
}
