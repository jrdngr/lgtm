import "./frame/boot";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { installErrorLogging, loadLogLevel } from "./core/log";
import { screenChanged } from "./core/scale";
import { Crash } from "./ui/Crash";
import "./styles.css";
import { installContextMenus } from "./ui/contextMenu";
import { installDropGuard } from "./ui/dropGuard";
import { installFonts } from "./ui/fonts";

installErrorLogging();
loadLogLevel();
installContextMenus();
installDropGuard();
installFonts();
getCurrentWindow().onScaleChanged(screenChanged);

const root = createRoot(document.getElementById("root") as HTMLElement);
window.lgtmTeardown = () => root.unmount();
root.render(
  <StrictMode>
    <Crash>
      <App />
    </Crash>
  </StrictMode>,
);
