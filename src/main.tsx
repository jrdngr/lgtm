import "./frame/boot";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { installErrorLogging, loadLogLevel } from "./core/log";
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

const root = createRoot(document.getElementById("root") as HTMLElement);
window.lgtmTeardown = () => root.unmount();
root.render(
  <StrictMode>
    <Crash>
      <App />
    </Crash>
  </StrictMode>,
);
