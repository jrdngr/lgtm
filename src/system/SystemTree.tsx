import { useMemo, useState } from "react";
import type { SystemDto } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { commands } from "../ipc";
import { useSendFromMenu } from "../review/useSendText";
import { PipeButton } from "../terminal/PipeButton";
import { GroupHead } from "../ui/GroupHead";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import {
  componentId,
  containerColors,
  containerId,
  containersOf,
  externalId,
  personId,
} from "./scene";
import { showInSystem } from "./showInSystem";
import { useSystem } from "./useSystem";
import "./tree.css";

/** Components a container lists before "… N more". */
const LIST = 7;

/** The workspace's system as a tree: people, external systems, and containers with their
 * components. A row shows its node in the System tab. */
export function SystemTree() {
  const workspace = useWorkspace();
  const { map, error } = useSystem(workspace);
  const { sendText, picker } = useSendFromMenu(workspace);
  async function describe() {
    const r = await commands.describePrompt(workspace);
    if (r.status === "ok") sendText(r.data);
  }
  return (
    <div className="side-split">
      <SectionHeader title="System">
        <PipeButton onClick={describe}>Describe</PipeButton>
      </SectionHeader>
      {map ? <Tree map={map} /> : <Empty>{error ?? "Loading…"}</Empty>}
      {picker}
    </div>
  );
}

function Tree({ map }: { map: SystemDto }) {
  const workspace = useWorkspace();
  const [selected, setSelected] = useState<string>();
  const [shut, setShut] = useState<Set<string>>(() => new Set(["people", "externals"]));
  const [more, setMore] = useState<Set<string>>(() => new Set());
  const colors = useMemo(() => containerColors(map), [map]);
  const stale = new Set(map.stale);
  const people = map.description?.people ?? [];
  const externals = map.description?.externals ?? [];
  const containers = containersOf(map).sort((a, b) => b.files - a.files);

  const toggle = (set: Set<string>, id: string) => {
    const next = new Set(set);
    if (!next.delete(id)) next.add(id);
    return next;
  };
  const show = (id: string) => {
    setSelected(id);
    showInSystem(workspace, id);
  };
  const row = (id: string, content: React.ReactNode, depth = 0) => (
    <button
      key={id}
      type="button"
      className={`sys-tr${depth ? " l1" : ""}${selected === id ? " sel" : ""}`}
      onClick={() => show(id)}
    >
      {content}
    </button>
  );
  const group = (key: string, name: string, count: number, children: React.ReactNode) => (
    <>
      <GroupHead
        name={name}
        count={count}
        shut={shut.has(key)}
        onToggle={() => setShut((s) => toggle(s, key))}
      />
      {!shut.has(key) && children}
    </>
  );

  return (
    <div className="side-split-body sys-tree">
      {map.description?.summary && <div className="sys-tree-sum">{map.description.summary}</div>}
      {!!people.length &&
        group(
          "people",
          "People",
          people.length,
          people.map((p) => row(personId(p.name), <span className="sys-tr-name">{p.name}</span>)),
        )}
      {!!externals.length &&
        group(
          "externals",
          "External systems",
          externals.length,
          externals.map((x) =>
            row(externalId(x.name), <span className="sys-tr-name">{x.name}</span>),
          ),
        )}
      {group(
        "containers",
        "Containers",
        containers.length,
        containers.map((c) => {
          const comps = map.components
            .filter((k) => k.container === c.id)
            .sort((a, b) => b.files - a.files);
          const open = !shut.has(`c:${c.id}`) && comps.length > 0;
          const listed = more.has(c.id) ? comps : comps.slice(0, LIST);
          return (
            <div key={c.id}>
              <div className={`sys-tr${selected === containerId(c.id) ? " sel" : ""}`}>
                <button
                  type="button"
                  className="sys-tr-chev"
                  aria-label={open ? "Collapse" : "Expand"}
                  disabled={!comps.length}
                  onClick={() => setShut((s) => toggle(s, `c:${c.id}`))}
                >
                  {comps.length ? (open ? "▾" : "▸") : ""}
                </button>
                <button
                  type="button"
                  className="sys-tr-main"
                  onClick={() => show(containerId(c.id))}
                >
                  <span
                    className="sys-tr-sw"
                    style={{ background: colors.get(c.id) ?? "#8b8d93" }}
                  />
                  <span className="sys-tr-name">{c.name}</span>
                  {c.tech && <span className="sys-tr-tech">{c.tech}</span>}
                  {c.files > 0 && <span className="sys-tr-n">{c.files} files</span>}
                </button>
              </div>
              {open && (
                <>
                  {listed.map((k) =>
                    row(
                      componentId(k.id),
                      <>
                        <span className="sys-tr-name">{k.name}</span>
                        {stale.has(k.id) && <span className="sys-tr-stale">● stale</span>}
                        <span className="sys-tr-n">{k.files}</span>
                      </>,
                      1,
                    ),
                  )}
                  {comps.length > LIST && !more.has(c.id) && (
                    <button
                      type="button"
                      className="sys-tr l1"
                      onClick={() => setMore((s) => toggle(s, c.id))}
                    >
                      <span className="sys-tr-name">… {comps.length - LIST} more</span>
                    </button>
                  )}
                </>
              )}
            </div>
          );
        }),
      )}
    </div>
  );
}
