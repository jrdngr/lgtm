import type { ReactNode } from "react";

/** The top line: what's selected, its pills, and ✕. */
export function Top({
  what,
  onClose,
  mono = true,
  children,
}: {
  what: ReactNode;
  onClose(): void;
  mono?: boolean;
  children?: ReactNode;
}) {
  return (
    <>
      <div className="adtop">
        <span className={`awhat${mono ? "" : " ui"}`}>{what}</span>
        <button type="button" className="aclose" aria-label="Close" onClick={onClose}>
          ✕
        </button>
      </div>
      {children && <div className="adtop">{children}</div>}
    </>
  );
}

export const Pill = ({ tone, children }: { tone: string; children: ReactNode }) => (
  <span className={`apill ${tone}`}>{children}</span>
);

export const Sec = ({ children }: { children: ReactNode }) => (
  <div className="adsec">{children}</div>
);

/** A clickable place in the code. */
export function Site({ loc, code, onClick }: { loc: string; code?: string; onClick(): void }) {
  return (
    <button type="button" className="asite" onClick={onClick}>
      <div className="aloc">{loc}</div>
      {code && <pre>{code}</pre>}
    </button>
  );
}
