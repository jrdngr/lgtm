import { useEffect, useState } from "react";
import type { SysSymbolDto, SystemDto } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { commands, unwrap } from "../ipc";

interface Props {
  /** Asks again when the map reloads. */
  map: SystemDto;
  component: string;
  onFile(path: string, line?: number): void;
}

/** The component's symbols other components use most. */
export function MostUsed({ map, component, onFile }: Props) {
  const workspace = useWorkspace();
  const [found, setFound] = useState<{ component: string; symbols: SysSymbolDto[] }>();

  // biome-ignore lint/correctness/useExhaustiveDependencies: a reloaded map asks again
  useEffect(() => {
    let live = true;
    unwrap(commands.systemMostUsed(workspace, component)).then(
      (symbols) => live && setFound({ component, symbols }),
      () => live && setFound({ component, symbols: [] }),
    );
    return () => {
      live = false;
    };
  }, [workspace, component, map]);

  const symbols = found?.component === component ? found.symbols : [];
  if (!symbols.length) return null;
  return (
    <section className="sys-sec">
      <h4>Most used</h4>
      {symbols.map((s) => (
        <button
          key={`${s.path}:${s.name}`}
          type="button"
          className="sys-li mono"
          onClick={() => onFile(s.path, s.line)}
        >
          <span>{s.name}</span>
          <span className="sys-n">{s.users}</span>
        </button>
      ))}
    </section>
  );
}
