import { useEffect, useState } from "react";
import { commands, type RepoInfo } from "./ipc";
import { Start } from "./Start";
import { ToolCheck } from "./tools/ToolCheck";
import { Workbench } from "./workbench/Workbench";

export function App() {
  const [repo, setRepo] = useState<RepoInfo | null | undefined>();

  useEffect(() => {
    commands.currentRepo().then(setRepo);
  }, []);

  if (repo === undefined) return null;
  return <ToolCheck>{repo ? <Workbench repo={repo} /> : <Start />}</ToolCheck>;
}
