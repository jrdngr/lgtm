import type { DesignLevel } from "./designScene";

/** The canvas key of a design level: its kind, and the container or component it shows. */
export function viewKey(level: DesignLevel): string {
  if (level.kind === "components") return `components/${level.container}`;
  if (level.kind === "interfaces") return `interfaces/${level.component}`;
  return level.kind;
}

/** The design level a canvas key draws. */
export function levelOfKey(key: string): DesignLevel {
  const [kind, id = ""] = key.split(/\/(.*)/s);
  if (kind === "components") return { kind, container: id };
  if (kind === "interfaces") return { kind, component: id };
  return { kind: kind === "context" ? "context" : "containers" };
}
