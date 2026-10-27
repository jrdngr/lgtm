import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef } from "react";
import type { WorkspaceDto } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { closing, confirmAll } from "../editor/unsaved";
import { loadedPr, prOf, subscribePrs } from "../github/pr";
import { commands } from "../ipc";
import { roleOf, subscribeTerminals } from "../terminal/registry";
import type { CtxEntry } from "../ui/contextMenu";
import { confirmAction } from "../ui/prompt";
import { activity, clearFinished, subscribeActivity } from "../workspaces/activity";
import { requestAsk } from "../workspaces/addRequests";
import { agentName, openAgent } from "../workspaces/agentTabs";
import { arrange, pinsNow, subscribePins } from "../workspaces/pins";
import { workspaceMenu } from "../workspaces/workspaceMenu";
import { frameRepo } from "./boot";
import { type Subtree, shell, type TreeRow } from "./shell";

function treeRow(w: WorkspaceDto): TreeRow {
  const pr = loadedPr(w.name);
  const a = activity(w.name);
  return {
    w,
    pr: pr && !pr.stack ? { number: pr.number, title: pr.title } : null,
    a: {
      state: a.state,
      agents: a.agents.map((g) => ({
        ...g,
        name: agentName(w.name, g),
        role: g.tab ? roleOf(w.name, g.tab) : undefined,
      })),
    },
    color: pinsNow().colors[workspaceKey(w.name)],
  };
}

/** The repo's rows, as the other repos' frames draw them. */
export function subtree(list: WorkspaceDto[]): Subtree {
  const { top, rest } = arrange(list, (w) => workspaceKey(w.name), pinsNow().pinned);
  return { top: top.map(treeRow), rest: rest.map(treeRow) };
}

/** Adds a repo to the project from a folder the user picks, offering `jj git init` for a
 * folder that isn't a jj repo yet, and shows it. */
export async function addRepo(onError: (e: string) => void) {
  const s = shell();
  const path = await open({ directory: true, title: "Add Repository" });
  if (!s || typeof path !== "string") return;
  let r = await commands.addRepo(path);
  if (r.status === "error") {
    const root = await commands.initRoot(path);
    if (root === null || !(await confirmAction(`${root} isn't a jj repo.`, "Run jj git init")))
      return onError(r.error);
    const init = await commands.initJj(root);
    if (init.status === "error") return onError(init.error);
    r = await commands.addRepo(root);
    if (r.status === "error") return onError(r.error);
  }
  if (!r.data) return;
  await s.refresh();
  s.show(r.data.label);
}

interface Frame {
  workspaces: WorkspaceDto[] | undefined;
  select(ws: WorkspaceName): void;
  sleep(ws: WorkspaceName): void;
  /** Opens the left panel on the Workspaces view. */
  showPanel(): void;
  onError(e: string): void;
}

/** Makes this frame's repo part of its project window: its rows go to the other frames, and
 * they can select, open menus on and delete its workspaces. */
export function useProjectFrame(frame: Frame) {
  const ref = useRef(frame);
  ref.current = frame;

  useEffect(() => {
    const s = shell();
    const label = frameRepo;
    if (!s || !label) return;
    const here = (ws?: WorkspaceName) => s.show(label, ws);
    const ask = (kind: "delete" | "locate", ws: string) => {
      here();
      ref.current.showPanel();
      requestAnimationFrame(() => requestAnimationFrame(() => requestAsk({ kind, ws })));
    };
    function menu(ws: WorkspaceName): CtxEntry[] {
      const list = ref.current.workspaces ?? [];
      const w = list.find((x) => x.name === ws);
      if (!w) return [];
      const key = workspaceKey(ws);
      const pins = pinsNow();
      const awake = list.filter((x) => !x.asleep).length;
      const a = activity(ws);
      return workspaceMenu({
        w,
        pinned: pins.pinned.includes(key),
        color: pins.colors[key],
        pr: loadedPr(ws),
        dot: a.state === "done" || a.state === "failed",
        open: () => here(ws),
        onSleep:
          awake > 1
            ? () => {
                here();
                ref.current.sleep(ws);
              }
            : undefined,
        onDelete: () => ws !== null && ask("delete", ws),
        onLocate: () => ws !== null && ask("locate", ws),
        onClearDot: () => clearFinished(ws),
      });
    }
    const unregister = s.register(label, {
      select: (ws) => ref.current.select(ws),
      menu,
      openAgent(ws, agent) {
        ref.current.select(ws);
        openAgent(ws, agent);
      },
      askDelete: (ws) => ask("delete", ws),
      confirmAll,
      async closing() {
        const { files, programs } = await closing();
        const list = ref.current.workspaces ?? [];
        const title = (key: string) =>
          list.find((w) => workspaceKey(w.name) === key)?.title ?? (key || "default");
        return {
          files: files.map((f) => ({ ...f, workspace: title(f.workspace) })),
          programs: programs.map((p) => ({ ...p, workspace: title(p.workspace) })),
        };
      },
      addRepo: () => addRepo((e) => ref.current.onError(e)),
    });
    // Coalesced to one per frame: activity and terminals change in bursts.
    let queued = 0;
    const publish = () => {
      if (queued) return;
      queued = requestAnimationFrame(() => {
        queued = 0;
        s.publish(label, subtree(ref.current.workspaces ?? []));
      });
    };
    publish();
    const subs = [
      subscribeActivity(publish),
      subscribePrs(publish),
      subscribePins(publish),
      subscribeTerminals(publish),
    ];
    publishRef.current = publish;
    return () => {
      cancelAnimationFrame(queued);
      for (const stop of subs) stop();
      unregister();
    };
  }, []);

  const publishRef = useRef<() => void>(undefined);
  const list = frame.workspaces;
  useEffect(() => {
    for (const w of list ?? []) prOf(w.name);
    publishRef.current?.();
  }, [list]);
}
