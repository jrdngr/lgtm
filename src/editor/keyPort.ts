/** Where an HTML tab takes the page's keys from: a port the tab hands the frame each time the
 * iframe fires load, which a page can't fake. The frame script's message listener is the
 * page's first and keeps the port from the page's scripts, so keys come only from the frame
 * script of the page now showing. */
export interface KeyPort {
  /** The frame loaded a page: drops the last port and hands a new one over with `post`. */
  connect(post: (port: MessagePort) => void): void;
  /** Drops the port. */
  close(): void;
}

export function keyPort(onKey: (m: Record<string, unknown>) => void): KeyPort {
  let port: MessagePort | undefined;
  const close = () => {
    port?.close();
    port = undefined;
  };
  return {
    connect(post) {
      close();
      const { port1, port2 } = new MessageChannel();
      port = port1;
      port1.onmessage = (e) => {
        if (e.data && typeof e.data === "object") onKey(e.data);
      };
      post(port2);
    },
    close,
  };
}
