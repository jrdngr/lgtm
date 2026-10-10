import { useEffect, useState } from "react";
import { commands, type RepoInfo } from "./ipc";
import { Start } from "./Start";
import { Workbench } from "./workbench/Workbench";

export function App() {
  const [repo, setRepo] = useState<RepoInfo | null | undefined>();

  useEffect(() => {
    commands.currentRepo().then(setRepo);
  }, []);

  if (repo === undefined) return null;
  return repo ? <Workbench repo={repo} /> : <Start />;
}
