import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { installErrorLogging, loadLogLevel } from "./core/log";
import { Crash } from "./ui/Crash";
import "./styles.css";
import { installContextMenus } from "./ui/contextMenu";
import { installDropGuard } from "./ui/dropGuard";

installErrorLogging();
loadLogLevel();
installContextMenus();
installDropGuard();

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <Crash>
      <App />
    </Crash>
  </StrictMode>,
);
