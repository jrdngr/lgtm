/** Markdown's extended syntax beyond GitHub's: `==highlight==`, `~sub~`, `^sup^` and
 * heading IDs (`# Title {#id}`), as a remark plugin. */

interface Node {
  type: string;
  value?: string;
  children?: Node[];
  data?: { hName?: string; hProperties?: Record<string, string> };
}

const INLINE = /==([^=\s](?:[^=]*[^=\s])?)==|~([^~\s]+)~|\^([^^\s]+)\^/g;
const TAGS = ["mark", "sub", "sup"] as const;

/** `text` split into text and `<mark>`, `<sub>`, `<sup>` nodes. */
export function splitInline(text: string): Node[] {
  const out: Node[] = [];
  let at = 0;
  for (const m of text.matchAll(INLINE)) {
    const i = [m[1], m[2], m[3]].findIndex((g) => g !== undefined);
    if (m.index > at) out.push({ type: "text", value: text.slice(at, m.index) });
    out.push({
      type: TAGS[i],
      children: [{ type: "text", value: m[i + 1] }],
      data: { hName: TAGS[i] },
    });
    at = m.index + m[0].length;
  }
  if (at < text.length) out.push({ type: "text", value: text.slice(at) });
  return out;
}

function walk(node: Node) {
  if (node.type === "heading") {
    const last = node.children?.[node.children.length - 1];
    const id = last?.type === "text" ? /\s*\{#([\w-]+)\}\s*$/.exec(last.value ?? "") : null;
    if (last && id) {
      last.value = last.value?.slice(0, id.index);
      node.data = { ...node.data, hProperties: { ...node.data?.hProperties, id: id[1] } };
    }
  }
  if (!node.children || node.type === "inlineCode" || node.type === "code") return;
  node.children = node.children.flatMap((child) => {
    if (child.type !== "text" || !child.value) {
      walk(child);
      return [child];
    }
    return splitInline(child.value);
  });
}

export function remarkExtras() {
  return (tree: Node) => walk(tree);
}
