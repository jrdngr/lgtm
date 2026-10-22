import type { SymbolInfoDto } from "../bindings";
import { type FlatFile, markKey } from "../review/model";

/** Symbol Information by the `markKey` of the row holding its definition's line. */
export function infoRows(files: FlatFile[], infos: SymbolInfoDto[]): Map<string, SymbolInfoDto[]> {
  const out = new Map<string, SymbolInfoDto[]>();
  if (!infos.length) return out;
  files.forEach((f, file) => {
    const here = infos.filter((i) => i.path === f.dto.path);
    if (!here.length) return;
    f.rows.forEach((r, row) => {
      if (r.kind === "removed") return;
      const at = here.filter((i) => i.line === r.newLine);
      if (at.length) out.set(markKey(file, row), at);
    });
  });
  return out;
}
