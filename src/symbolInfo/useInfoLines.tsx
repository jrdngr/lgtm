import { useMemo } from "react";
import type { WorkspaceName } from "../core/workspace";
import { type FlatFile, markKey } from "../review/model";
import { InfoBlock, InfoIcon } from "./InfoBlock";
import { infoRows } from "./rows";
import { infoKey, useFolded, useSymbolInfos } from "./store";

/** Symbol Information placed on a diff's rows: blocks under the open ones, by row `markKey`,
 * and gutter icons for folded ones. */
export function useInfoLines(ws: WorkspaceName, files: FlatFile[]) {
  const paths = useMemo(() => files.map((f) => f.dto.path), [files]);
  const infos = useSymbolInfos(ws, paths);
  const folded = useFolded();
  return useMemo(() => {
    const at = infoRows(files, infos);
    const byKey = new Map(infos.map((i) => [infoKey(i), i]));
    const open = new Map<string, string[]>();
    for (const [row, list] of at) {
      const keys = list.map(infoKey).filter((k) => !folded.has(k));
      if (keys.length) open.set(row, keys);
    }
    return {
      open,
      render(key: string) {
        const info = byKey.get(key);
        return info ? <InfoBlock ws={ws} info={info} /> : null;
      },
      flag(file: number, row: number) {
        const shut = at.get(markKey(file, row))?.filter((i) => folded.has(infoKey(i)));
        return shut?.length ? <InfoIcon info={shut[0]} /> : null;
      },
    };
  }, [ws, files, infos, folded]);
}
