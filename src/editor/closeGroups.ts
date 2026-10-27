/** Where a row is, when the close dialog asks for a window of several repos. */
export interface Where {
  repo: string;
  workspace: string;
}

/** Rows in order under their repo and then their workspace, each name shown once. */
export function grouped<T extends { where?: Where }>(rows: T[]) {
  const repos: { repo: string; workspaces: { workspace: string; rows: T[] }[] }[] = [];
  for (const row of rows) {
    const { repo, workspace } = row.where ?? { repo: "", workspace: "" };
    let r = repos.find((x) => x.repo === repo);
    if (!r) {
      r = { repo, workspaces: [] };
      repos.push(r);
    }
    let w = r.workspaces.find((x) => x.workspace === workspace);
    if (!w) {
      w = { workspace, rows: [] };
      r.workspaces.push(w);
    }
    w.rows.push(row);
  }
  return repos;
}
