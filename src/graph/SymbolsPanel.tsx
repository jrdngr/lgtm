import type { GraphDto, GraphNode } from "../bindings";
import { useCurrentReview } from "../review/current";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { nodeDone, nodeLabel, nodeRisk, STATUS_SIGN, showNode } from "./nodes";
import { useGraph } from "./useGraph";
import "./SymbolsPanel.css";

/** Callers of each node, by id. */
function callersOf(graph: GraphDto) {
  const out = new Map<number, GraphNode[]>();
  for (const e of graph.edges) {
    const from = graph.nodes[e.from];
    if (from?.status !== "caller") continue;
    out.set(e.to, [...(out.get(e.to) ?? []), from]);
  }
  return out;
}

function Shell({ meta, children }: { meta?: string; children: React.ReactNode }) {
  return (
    <div className="side-split">
      <SectionHeader title="Symbols" meta={meta} />
      {children}
    </div>
  );
}

/** The current review's changed symbols by file. Click one to show it. */
export function SymbolsPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const { graph, error, loading } = useGraph(review);
  if (!review)
    return (
      <Shell>
        <Empty>No review open</Empty>
      </Shell>
    );
  if (error)
    return (
      <Shell>
        <Empty>
          <span className="error">{error}</span>
        </Empty>
      </Shell>
    );
  if (loading || !graph)
    return (
      <Shell>
        <Empty>Finding symbols…</Empty>
      </Shell>
    );
  const changed = graph.nodes.filter((n) => n.status !== "caller");
  if (changed.length === 0)
    return (
      <Shell>
        <Empty>No changed symbols</Empty>
      </Shell>
    );
  const callers = callersOf(graph);
  const byPath = new Map<string, GraphNode[]>();
  for (const n of changed) byPath.set(n.path, [...(byPath.get(n.path) ?? []), n]);
  return (
    <Shell meta={`${changed.length} changed`}>
      <div className="side-split-body symbol-list">
        {[...byPath].map(([path, nodes]) => (
          <section key={path}>
            <div className="symbol-file" title={path}>
              <bdi dir="ltr">{path}</bdi>
            </div>
            <ul>
              {nodes.map((n) => {
                const done = nodeDone(review, n) ?? 0;
                const users = callers.get(n.id) ?? [];
                const risk = nodeRisk(review, n);
                return (
                  <li key={n.id}>
                    <button
                      type="button"
                      className={`symbol-${n.status}`}
                      onClick={() => showNode(review, n, workspace)}
                      title={users.length ? `Used by ${users.map(nodeLabel).join(", ")}` : n.path}
                    >
                      <span className="symbol-sign">{STATUS_SIGN[n.status]}</span>
                      <span className="symbol-name">{nodeLabel(n)}</span>
                      {risk && <span className={`risk-dot risk-${risk}`} />}
                      {users.length > 0 && (
                        <span className="pill" title={`${users.length} unchanged callers`}>
                          ← {users.length}
                        </span>
                      )}
                      {done === 1 ? (
                        <span className="pill green">✓</span>
                      ) : (
                        <span className="pill">{Math.floor(done * 100)}%</span>
                      )}
                    </button>
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
      </div>
    </Shell>
  );
}
