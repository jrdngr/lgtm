import type { DesignDto, DesignMapDto } from "../bindings";
import type { DesignNode } from "./designScene";

/** What a node is called, and its id in the design's map. */
export function nodeName(n: DesignNode): { name: string; id: string } {
  switch (n.kind) {
    case "person":
    case "external":
      return { name: n.name, id: n.name };
    case "container":
      return { name: n.c.name, id: n.c.id };
    case "component":
      return { name: n.k.name, id: n.k.id };
    case "interface":
      return {
        name: `${n.k.name} ${n.k.interfaces[n.at]?.signature || n.k.interfaces[n.at]?.name}`,
        id: n.k.id,
      };
    case "code":
      return { name: n.name, id: n.id };
  }
}

const where = (d: DesignDto) =>
  `the design "${d.name}" (spec files ${d.files.join(", ")} at revision ${d.revision})`;

/** Asks about a node of the design, or the whole design. */
export function askDesignPrompt(d: DesignDto, n: DesignNode | undefined): string {
  const what = n ? `the ${nodeName(n).name} part of ${where(d)}` : where(d);
  return `Explain ${what}: what it does, how it fits with the rest, and how it compares to the code. Read the spec with \`jj file show -r ${d.revision} <path>\`. Keep it short.`;
}

/** Asks for the spec to be brought in line with the code where they differ. */
export function updateSpecPrompt(d: DesignDto, m: DesignMapDto, n: DesignNode): string {
  const { name, id } = nodeName(n);
  const diffs = m.differences
    .filter((x) => x.node === id)
    .map((x) => `- ${x.text}${x.path ? ` (${x.path}${x.line ? `:${x.line}` : ""})` : ""}`)
    .join("\n");
  return `The code differs from ${where(d)} at ${name}:\n${diffs}\n\nUpdate the spec files so they describe what the code does there. Make the change in revision ${d.revision} (\`jj edit ${d.revision}\`, or \`jj new\` on it and squash) unless the working copy is already there. Don't change the code.`;
}
