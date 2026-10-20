import type { LookDto, SystemDto } from "../bindings";
import { tint } from "./colors";
import { DeltaCounts } from "./Delta";
import { isChanged, type Lens } from "./lens";
import { componentId } from "./scene";

const SEV = { high: ["hi", "HIGH"], medium: ["md", "MED"], low: ["lo", "LOW"] } as const;

/** The review's open Look items on a node. */
export function LookList({ items, onLook }: { items: LookDto[]; onLook(l: LookDto): void }) {
  if (!items.length) return null;
  return (
    <section className="sys-sec">
      <h4>
        <span>Look</span>
        <span>{items.length}</span>
      </h4>
      {items.map((l) => (
        <button key={l.key} type="button" className="sys-look" onClick={() => onLook(l)}>
          <span className={`sys-sev ${SEV[l.severity][0]}`}>{SEV[l.severity][1]}</span>
          {l.title}
        </button>
      ))}
    </section>
  );
}

/** The files the review changes in a component, with their added and removed rows. */
export function ChangedFiles({
  lens,
  component,
  onFile,
}: {
  lens: Lens;
  component: string;
  onFile(path: string): void;
}) {
  const files = lens.files.get(component) ?? [];
  if (!files.length) return null;
  return (
    <section className="sys-sec">
      <h4>
        <span>Changed files</span>
        <span>{files.length}</span>
      </h4>
      {files.map((f) => (
        <button key={f.path} type="button" className="sys-li mono" onClick={() => onFile(f.path)}>
          <span>{f.path.slice(f.path.lastIndexOf("/") + 1)}</span>
          <span className="sys-n">
            {!!f.add && <span className="add">+{f.add}</span>}
            {!!f.del && <span className="del">−{f.del}</span>}
          </span>
        </button>
      ))}
    </section>
  );
}

/** The components of a container the review changes. */
export function ChangedInside({
  map,
  lens,
  container,
  color,
  onShow,
}: {
  map: SystemDto;
  lens: Lens;
  container: string;
  color: string;
  onShow(id: string): void;
}) {
  const comps = [...map.components, ...lens.removed]
    .filter((k) => k.container === container && isChanged(lens.status.get(k.id)))
    .sort((a, b) => b.files - a.files);
  if (!comps.length) return null;
  return (
    <section className="sys-sec">
      <h4>
        <span>Changed inside</span>
        <span>{comps.length}</span>
      </h4>
      {comps.map((k, i) => (
        <button
          key={k.id}
          type="button"
          className="sys-li"
          onClick={() => onShow(componentId(k.id))}
        >
          <span className="sys-sw" style={{ background: tint(color, 0.7 - i * 0.1) }} />
          <span>{k.name}</span>
          <span className="sys-n delta">
            {lens.status.get(k.id) === "added" ? "new" : <DeltaCounts d={lens.delta.get(k.id)} />}
          </span>
        </button>
      ))}
    </section>
  );
}
