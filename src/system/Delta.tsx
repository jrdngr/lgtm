import type { Delta } from "./lens";

/** `+4 −1` in the diff's green and red; nothing for no change. */
export function DeltaCounts({ d, className }: { d: Delta | undefined; className?: string }) {
  if (!d || (!d.add && !d.del)) return null;
  return (
    <span className={`sys-dc${className ? ` ${className}` : ""}`}>
      {!!d.add && <span className="add">+{d.add}</span>}
      {!!d.del && <span className="del">−{d.del}</span>}
    </span>
  );
}
