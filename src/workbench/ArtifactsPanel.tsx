import { DrawingsPanel } from "../drawings/DrawingsPanel";
import { NotesPanel } from "../notes/NotesPanel";
import "./artifacts.css";

/** Notes, then drawings below a divider. */
export function ArtifactsPanel() {
  return (
    <div className="artifacts-panel">
      <NotesPanel />
      <hr className="artifacts-divider" />
      <DrawingsPanel />
    </div>
  );
}
