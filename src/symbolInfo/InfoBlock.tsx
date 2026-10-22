import type { SymbolInfoDto, SymbolKind } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { askSymbolInfo } from "../search/menu";
import { PipeButton } from "../terminal/PipeButton";
import { dismiss, infoKey, setFolded } from "./store";
import "./symbolInfo.css";

const KIND: Record<SymbolKind, string> = {
  function: "fn",
  class: "type",
  module: "module",
  section: "section",
};

interface Props {
  ws: WorkspaceName;
  info: SymbolInfoDto;
}

/** Symbol Information under its definition's line: what it does, its callers, what callers
 * must assume, and how it fails. Folds to `InfoIcon`. */
export function InfoBlock({ ws, info }: Props) {
  const fold = () => setFolded(infoKey(info), true);
  const empty = !info.description && !info.callers.length;
  return (
    <div className="si" onMouseDown={(e) => e.stopPropagation()}>
      <div className={`si-bar${empty ? " alone" : ""}`}>
        <button type="button" className="si-fold" onClick={fold}>
          ▾
        </button>
        <span className="si-name">{info.name}</span>
        <span className="si-kind">{KIND[info.kind]}</span>
        <span className="spacer" />
        {info.changed && <span className="si-changed">Changed</span>}
        {info.working ? (
          <span className="si-working">Working…</span>
        ) : (
          <PipeButton
            className="si-refresh"
            onClick={() => askSymbolInfo(ws, info.name, info.path)}
          >
            Refresh
          </PipeButton>
        )}
        <button type="button" className="si-close" onClick={() => dismiss(info)}>
          ×
        </button>
      </div>
      {info.description && (
        <Section title="Description">
          <p>{info.description}</p>
        </Section>
      )}
      {info.callers.length > 0 && (
        <Section title={`Callers · ${info.callers.length}`}>
          {info.callers.map((c) => (
            <div key={`${c.path}:${c.line}:${c.name}`} className="si-caller">
              <span className="si-who">{c.name}</span>
              <button type="button" className="si-loc" onClick={() => openFile(ws, c.path, c.line)}>
                {c.path}:{c.line}
              </button>
            </div>
          ))}
        </Section>
      )}
      <List title="Assumptions & invariants" items={info.assumptions} />
      <List title="Errors & side effects" items={info.errors} />
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="si-sec">
      <h3>{title}</h3>
      {children}
    </div>
  );
}

function List({ title, items }: { title: string; items: string[] }) {
  if (!items.length) return null;
  return (
    <Section title={title}>
      <ul>
        {items.map((t) => (
          <li key={t}>{t}</li>
        ))}
      </ul>
    </Section>
  );
}

/** Folded Symbol Information, left of its line's number. Dashed while the agent works. */
export function InfoIcon({ info }: { info: SymbolInfoDto }) {
  return (
    <button
      type="button"
      className={`si-icon${info.working ? " busy" : ""}`}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={() => setFolded(infoKey(info), false)}
    >
      i
    </button>
  );
}
