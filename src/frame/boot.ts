/** A project window shows each of its repos in a frame of its own, an iframe of this app at
 * `?repo=<label>`. A frame talks to the backend through the window's own Tauri bridge, with
 * every command marked with its repo, and hears the events sent to its repo. Imported before
 * anything else, so the bridge is in place before any module uses it. */

type Internals = Record<string, unknown> & {
  invoke(cmd: string, args?: unknown, options?: { headers?: Record<string, string> }): unknown;
  metadata: { currentWindow: { label: string }; currentWebview: { label: string } };
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: Internals;
  }
}

/** The label of the repo this frame shows, or null outside a frame. */
export const frameRepo: string | null =
  window.parent !== window ? new URLSearchParams(location.search).get("repo") : null;

/** Copies values the window's bridge checks by type, which it can't recognise when they were
 * made in this frame: byte arrays become plain arrays and maps plain objects. */
export function plain(v: unknown): unknown {
  if (v === null || typeof v !== "object") return v;
  if (ArrayBuffer.isView(v)) return Array.from(v as Uint8Array);
  if (Object.prototype.toString.call(v) === "[object ArrayBuffer]")
    return Array.from(new Uint8Array(v as ArrayBuffer));
  if (Object.prototype.toString.call(v) === "[object Map]")
    return Object.fromEntries([...(v as Map<string, unknown>)].map(([k, x]) => [k, plain(x)]));
  if (Array.isArray(v)) return v.map(plain);
  const proto = Object.getPrototypeOf(v);
  // Class instances, like channels, serialize themselves.
  if (proto !== null && proto.constructor?.name !== "Object") return v;
  return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, plain(x)]));
}

if (frameRepo) {
  const label = frameRepo;
  const parent = window.parent as Window;
  const bridge = parent.__TAURI_INTERNALS__ as Internals;
  const metadata = {
    currentWindow: bridge.metadata.currentWindow,
    currentWebview: { ...bridge.metadata.currentWebview, label },
  };
  const invoke = (cmd: string, args?: unknown, options?: { headers?: Record<string, string> }) =>
    bridge.invoke(cmd, plain(args), {
      ...options,
      headers: { ...options?.headers, "lgtm-repo": label },
    });
  // Over an empty object: the bridge's own properties can't be overridden by a proxy of it.
  window.__TAURI_INTERNALS__ = new Proxy({} as Internals, {
    get(_, key) {
      if (key === "metadata") return metadata;
      if (key === "invoke") return invoke;
      const v = Reflect.get(bridge, key);
      return typeof v === "function" ? v.bind(bridge) : v;
    },
    has: (_, key) => Reflect.has(bridge, key),
    set: (_, key, v) => Reflect.set(bridge, key, v),
  });
  // Events reach a frame's listeners through the window's event bridge.
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = parent.__TAURI_EVENT_PLUGIN_INTERNALS__;
}
