import { type ReactNode, useMemo, useState } from "react";
import type { SystemDto } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { GroupHead } from "../ui/GroupHead";
import { deltaText, isChanged, type Lens } from "./lens";
import {
  componentId,
  containerColors,
  containerId,
  containersOf,
  externalId,
  personId,
} from "./scene";
import { showInSystem, useSelectedNode } from "./showInSystem";
import { tourPath } from "./tours";
import "./tree.css";

/** Components a container lists before "… N more". */
const LIST = 7;

const toggle = (set: Set<string>, id: string) => {
  const next = new Set(set);
  if (!next.delete(id)) next.add(id);
  return next;
};

/** A folding group of the tree. */
function Group(p: {
  name: string;
  count: number;
  shut: boolean;
  onToggle(): void;
  children: ReactNode;
}) {
  return (
    <>
      <GroupHead name={p.name} count={p.count} shut={p.shut} onToggle={p.onToggle} />
      {!p.shut && p.children}
    </>
  );
}

interface Props {
  map: SystemDto;
  /** With a review open: only the containers, marked with what it changes. */
  lens?: Lens;
}

/** The workspace's system as a tree: its tours, people, external systems, and containers
 * with their components. A row shows its node in the Architecture tab. With a review, the
 * containers as a map of the change. */
export function SystemTree({ map, lens }: Props) {
  const workspace = useWorkspace();
  const selected = useSelectedNode(workspace);
  const [shut, setShut] = useState<Set<string>>(() => new Set(["tours", "people", "externals"]));
  const [more, setMore] = useState<Set<string>>(() => new Set());
  const colors = useMemo(() => containerColors(map), [map]);
  const stale = new Set(map.stale);
  const people = map.description?.people ?? [];
  const externals = map.description?.externals ?? [];
  const containers = containersOf(map).sort((a, b) => b.files - a.files);
  const fold = (key: string) => () => setShut((s) => toggle(s, key));

  const show = (id: string) => showInSystem(workspace, id);
  const row = (id: string, content: ReactNode, depth = 0) => (
    <button
      key={id}
      type="button"
      className={`sys-tr${depth ? " l1" : ""}${selected === id ? " sel" : ""}`}
      onClick={() => show(id)}
    >
      {content}
    </button>
  );

  const tree = containers.map((c) => {
    const all = [
      ...map.components.filter((k) => k.container === c.id),
      ...(lens?.removed ?? []).filter((k) => k.container === c.id),
    ].sort((a, b) => b.files - a.files);
    const changed = lens ? all.filter((k) => isChanged(lens.status.get(k.id))) : all;
    const comps = more.has(c.id) ? all : lens ? changed : all.slice(0, LIST);
    const hidden = all.length - comps.length;
    // With a review, containers it changes start open and the rest shut.
    const key = `c:${c.id}`;
    const open = (lens ? shut.has(key) === !changed.length : !shut.has(key)) && all.length > 0;
    const delta = changed.reduce(
      (d, k) => {
        const x = lens?.delta.get(k.id);
        return x ? { add: d.add + x.add, del: d.del + x.del } : d;
      },
      { add: 0, del: 0 },
    );
    const count = lens ? (
      changed.length ? (
        <span className="sys-tr-n chg">{deltaText(delta) || "changed"}</span>
      ) : (
        all.length > 0 && <span className="sys-tr-n">{all.length}</span>
      )
    ) : (
      c.files > 0 && <span className="sys-tr-n">{c.files} files</span>
    );
    return (
      <div key={c.id}>
        <div className={`sys-tr${selected === containerId(c.id) ? " sel" : ""}`}>
          <button
            type="button"
            className="sys-tr-chev"
            aria-label={open ? "Collapse" : "Expand"}
            disabled={!all.length}
            onClick={fold(key)}
          >
            {all.length ? (open ? "▾" : "▸") : ""}
          </button>
          <button type="button" className="sys-tr-main" onClick={() => show(containerId(c.id))}>
            <span className="sys-tr-sw" style={{ background: colors.get(c.id) ?? "#8b8d93" }} />
            <span className="sys-tr-name">{c.name}</span>
            {c.tech && <span className="sys-tr-tech">{c.tech}</span>}
            {count}
          </button>
        </div>
        {open && (
          <>
            {comps.map((k) => {
              const s = lens?.status.get(k.id);
              return row(
                componentId(k.id),
                <>
                  <span className="sys-tr-name">{k.name}</span>
                  {!lens && stale.has(k.id) && <span className="sys-tr-stale">● stale</span>}
                  {lens && isChanged(s) ? (
                    <span className={`sys-tr-n ${s === "added" ? "new" : "chg"}`}>
                      {s === "added"
                        ? "new"
                        : s === "removed"
                          ? "removed"
                          : deltaText(lens.delta.get(k.id))}
                    </span>
                  ) : (
                    <span className="sys-tr-n">{k.files}</span>
                  )}
                </>,
                1,
              );
            })}
            {hidden > 0 && (
              <button
                type="button"
                className="sys-tr l1"
                onClick={() => setMore((s) => toggle(s, c.id))}
              >
                <span className="sys-tr-name">
                  … {hidden} {lens ? "unchanged" : "more"}
                </span>
              </button>
            )}
          </>
        )}
      </div>
    );
  });

  if (lens) {
    return (
      <Group name="Map" count={containers.length} shut={shut.has("map")} onToggle={fold("map")}>
        {tree}
      </Group>
    );
  }
  return (
    <>
      {map.description?.summary && <div className="sys-tree-sum">{map.description.summary}</div>}
      {!!map.tours.length && (
        <Group
          name="Tours"
          count={map.tours.length}
          shut={shut.has("tours")}
          onToggle={fold("tours")}
        >
          {map.tours.map((t) => (
            <button
              key={t.id}
              type="button"
              className="sys-tr"
              onClick={() => show(`tour:${t.id}`)}
            >
              <span className="sys-tr-name">{t.title}</span>
              <span className="sys-tr-tech">{tourPath(map, colors, t)}</span>
              <span className="sys-tr-n">{t.steps.length}</span>
            </button>
          ))}
        </Group>
      )}
      {!!people.length && (
        <Group
          name="People"
          count={people.length}
          shut={shut.has("people")}
          onToggle={fold("people")}
        >
          {people.map((p) => row(personId(p.name), <span className="sys-tr-name">{p.name}</span>))}
        </Group>
      )}
      {!!externals.length && (
        <Group
          name="External systems"
          count={externals.length}
          shut={shut.has("externals")}
          onToggle={fold("externals")}
        >
          {externals.map((x) =>
            row(externalId(x.name), <span className="sys-tr-name">{x.name}</span>),
          )}
        </Group>
      )}
      <Group
        name="Containers"
        count={containers.length}
        shut={shut.has("containers")}
        onToggle={fold("containers")}
      >
        {tree}
      </Group>
    </>
  );
}
