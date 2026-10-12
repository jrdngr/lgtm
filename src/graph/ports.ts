import type { Tree } from "./tree";
import type { Link } from "./view";

/** A port on an open box's edge: links leave by an `out` port on the right and come in by an
 * `in` port on the left. Each port serves one box outside, so a box has one port per box it
 * connects to, whatever is inside. */
export interface Port {
  id: string;
  box: string;
  side: "out" | "in";
}

/** A link's way through the ports: the boxes and ports it passes, in order. */
export interface Route {
  link: Link;
  stops: string[];
}

export interface Routed {
  routes: Route[];
  ports: Map<string, Port>;
  /** Every step between two stops, once, as `from>to`. */
  segments: Map<string, { from: string; to: string; caller: boolean }>;
}

const ancestors = (tree: Tree, id: string) => {
  const out: string[] = [];
  for (let b = tree.boxes.get(id); b?.parent; b = tree.boxes.get(b.parent)) out.push(b.parent);
  return out;
};

/** Routes each link out of the open boxes around its start and into those around its end,
 * through one port per box and far end. */
export function route(tree: Tree, links: Link[]): Routed {
  const ports = new Map<string, Port>();
  const segments: Routed["segments"] = new Map();
  const port = (box: string, side: Port["side"], other: string) => {
    const id = `${side}:${box}>${other}`;
    if (!ports.has(id)) ports.set(id, { id, box, side });
    return id;
  };
  const routes = links.map((link): Route => {
    const up = ancestors(tree, link.from);
    const down = ancestors(tree, link.to);
    const common = up.find((a) => down.includes(a));
    const outer = (chain: string[], id: string) => {
      const i = common === undefined ? chain.length : chain.indexOf(common);
      return { inside: chain.slice(0, i), top: i > 0 ? chain[i - 1] : id };
    };
    const from = outer(up, link.from);
    const to = outer(down, link.to);
    const stops = [
      link.from,
      ...from.inside.map((b) => port(b, "out", to.top)),
      ...to.inside.reverse().map((b) => port(b, "in", from.top)),
      link.to,
    ];
    for (let i = 1; i < stops.length; i++) {
      const key = `${stops[i - 1]}>${stops[i]}`;
      const had = segments.get(key);
      segments.set(key, {
        from: stops[i - 1],
        to: stops[i],
        caller: (had?.caller ?? true) && link.caller,
      });
    }
    return { link, stops };
  });
  return { routes, ports, segments };
}
