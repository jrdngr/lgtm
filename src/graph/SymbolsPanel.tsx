import type { GraphDto, GraphNode } from "../bindings";
import { useCurrentReview } from "../review/current";
import { useWorkspace } from "../workspaces/context";
import { nodeDone, nodeLabel, nodeRisk, STATUS_SIGN, showNode } from "./nodes";
import { useGraph } from "./useGraph";
import "./graph.css";

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

/** The current review's changed symbols by file. Click one to show it. */
export function SymbolsPanel() {
  const review = useCurrentReview(useWorkspace());
  const { graph, error, loading } = useGraph(review);
  if (!review) return <div className="placeholder">No review open</div>;
  if (error) return <div className="placeholder error">{error}</div>;
  if (loading || !graph) return <div className="placeholder">Finding symbols…</div>;
  const changed = graph.nodes.filter((n) => n.status !== "caller");
  if (changed.length === 0) return <div className="placeholder">No changed symbols</div>;
  const callers = callersOf(graph);
  const byPath = new Map<string, GraphNode[]>();
  for (const n of changed) byPath.set(n.path, [...(byPath.get(n.path) ?? []), n]);
  return (
    <div className="symbol-list">
      <div className="tree-total">
        {changed.length} changed {changed.length === 1 ? "symbol" : "symbols"}
      </div>
      {[...byPath].map(([path, nodes]) => (
        <section key={path}>
          <div className="symbol-file" title={path}>
            {path}
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
                    className={`gnode-${n.status}`}
                    onClick={() => showNode(review, n)}
                    title={users.length ? `Used by ${users.map(nodeLabel).join(", ")}` : n.path}
                  >
                    <span className="gnode-sign">{STATUS_SIGN[n.status]}</span>
                    <span className="symbol-name">{nodeLabel(n)}</span>
                    {risk && <span className={`risk-dot risk-${risk}`} />}
                    {users.length > 0 && <span className="symbol-users">←{users.length}</span>}
                    <span className={`symbol-done${done === 1 ? " all" : ""}`}>
                      {Math.round(done * 100)}%
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </div>
  );
}
