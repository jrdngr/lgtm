import { useEffect, useMemo, useState } from "react";
import type { AgentActivity, Todo } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { decorate } from "../editor/decor";
import { HeatGlow } from "../editor/HeatGlow";
import { useCurrentReview } from "../review/current";
import { Icon } from "../ui/icons";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { elapsed, hotFiles, useAgentActivity } from "./activity";
import { Churn } from "./Churn";
import "./agent.css";

/** The Agent sidebar tab: the workspace's newest agent turn, its todo list, each edit's
 * churn, and the files it edited glowing as they're written. */
export function AgentPanel() {
  const workspace = useWorkspace();
  const activity = useAgentActivity(workspace);
  return (
    <div className="agent-panel">
      <SectionHeader title="Agent" />
      {activity ? <Activity activity={activity} /> : <Empty>No agent turn</Empty>}
    </div>
  );
}

function Activity({ activity }: { activity: AgentActivity }) {
  const done = activity.todos.filter((t) => t.status === "completed").length;
  return (
    <div className="agent-body">
      <TurnLine activity={activity} />
      {activity.todos.length > 0 && (
        <>
          <SectionHeader title="Todo">
            <span className="section-meta">
              {done}/{activity.todos.length}
            </span>
          </SectionHeader>
          <ul className="agent-todo">
            {activity.todos.map((t, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: a todo list has no ids
              <TodoRow key={i} todo={t} />
            ))}
          </ul>
        </>
      )}
      <SectionHeader title="Churn">
        <span className="section-meta">
          {activity.edits.length} edit{activity.edits.length === 1 ? "" : "s"}
        </span>
      </SectionHeader>
      <Churn edits={activity.edits} />
      <HotFiles activity={activity} />
    </div>
  );
}

function TurnLine({ activity }: { activity: AgentActivity }) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!activity.running) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [activity.running]);
  const start = activity.startedAt ?? now;
  const end = activity.running ? now : (activity.endedAt ?? now);
  return (
    <div className="agent-turn">
      {activity.running && <span className="agent-running" />}
      <span className="agent-turn-icon">
        <Icon name="terminal" size={14} />
      </span>
      <span className="agent-prompt">{activity.prompt?.split("\n")[0] ?? "Agent turn"}</span>
      <span className="agent-time">{elapsed(end - start)}</span>
    </div>
  );
}

function TodoRow({ todo }: { todo: Todo }) {
  return (
    <li className={todo.status}>
      <span className="agent-box" />
      <span>{todo.content}</span>
    </li>
  );
}

function HotFiles({ activity }: { activity: AgentActivity }) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const files = review?.files;
  const decor = useMemo(() => decorate(files?.map((f) => f.dto) ?? []), [files]);
  const hot = hotFiles(activity.edits);
  if (hot.length === 0) return null;
  return (
    <>
      <SectionHeader title="Files">
        <span className="section-meta">{hot.length}</span>
      </SectionHeader>
      <ul className="agent-files">
        {hot.map((f) => {
          const cut = f.path.lastIndexOf("/") + 1;
          const d = decor.byFile.get(f.path);
          return (
            <li key={f.path} title={f.path}>
              <HeatGlow at={f.last} decor={d} />
              <span className="agent-file">
                <span className="agent-dir">{f.path.slice(0, cut)}</span>
                <span className={`decor-${d ?? "modified"}`}>{f.path.slice(cut)}</span>
              </span>
              <span className="agent-count">{f.edits}</span>
            </li>
          );
        })}
      </ul>
    </>
  );
}
