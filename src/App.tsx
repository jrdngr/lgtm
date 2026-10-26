import { useEffect, useState } from "react";
import { frameRepo } from "./frame/boot";
import { commands, type RepoInfo } from "./ipc";
import { Start } from "./Start";
import { Shell } from "./shell/Shell";
import { loadShell } from "./shell/store";
import { ToolCheck } from "./tools/ToolCheck";
import { Workbench } from "./workbench/Workbench";

/** One repo, in its frame of a project window. */
function Frame() {
  const [repo, setRepo] = useState<RepoInfo | null>();

  useEffect(() => {
    commands.currentRepo().then(setRepo);
  }, []);

  if (!repo) return null;
  return (
    <ToolCheck>
      <Workbench repo={repo} />
    </ToolCheck>
  );
}

/** A window: a project's, or the start window. */
function Window() {
  const [project, setProject] = useState<boolean>();

  useEffect(() => {
    loadShell().then(setProject);
  }, []);

  if (project === undefined) return null;
  return project ? <Shell /> : <Start />;
}

export function App() {
  return frameRepo ? <Frame /> : <Window />;
}
