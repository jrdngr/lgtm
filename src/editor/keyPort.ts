/** Where an HTML tab takes the page's keys from: a port the frame script makes before any of
 * the page's scripts run and offers first. The tab takes the first offer from a new frame, and
 * after that only the first once the page holding the port says it's leaving, which it proves
 * with a token sent over the port. A page's scripts can offer ports too, but only after the
 * frame script, so theirs are refused, or taken as the page unloads and die with it. */
export interface KeyPort {
  /** The frame offered `port`. */
  offer(port: MessagePort): void;
  /** The frame said its page is leaving, with `token`. */
  gone(token: unknown): void;
  /** A new frame: its first offer is taken. */
  reset(): void;
}

export function keyPort(
  onKey: (m: Record<string, unknown>) => void,
  newToken: () => string = () => crypto.randomUUID(),
): KeyPort {
  let open = true;
  let port: MessagePort | undefined;
  let token: string | undefined;
  const drop = () => {
    port?.close();
    port = undefined;
    token = undefined;
  };
  return {
    offer(p) {
      if (!open) {
        p.close();
        return;
      }
      open = false;
      drop();
      port = p;
      token = newToken();
      p.onmessage = (e) => {
        if (e.data && typeof e.data === "object") onKey(e.data);
      };
      p.postMessage(token);
    },
    gone(t) {
      if (token === undefined || t !== token) return;
      drop();
      open = true;
    },
    reset() {
      drop();
      open = true;
    },
  };
}
