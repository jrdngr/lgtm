import { dockOf } from "../core/docks";
import { openArchitecture } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import { useSendFromMenu } from "../review/useSendText";
import { DesignForm } from "./DesignForm";
import { closeDesignForm, designsChanged, designText, showDesign, useEditing } from "./designs";

/** The design form, wherever it was opened from. A saved design shows in the Architecture
 * tab; a generated one also goes to a terminal agent. */
export function DesignFormHost({ workspace }: { workspace: WorkspaceName }) {
  const editing = useEditing(workspace);
  const { sendText, picker } = useSendFromMenu(workspace);
  return (
    <>
      {editing && (
        <DesignForm
          key={editing.design?.id ?? "new"}
          workspace={workspace}
          design={editing.design}
          onCancel={closeDesignForm}
          onDone={async (saved, generate) => {
            const text = generate ? await designText(workspace, saved.id) : undefined;
            closeDesignForm();
            designsChanged();
            showDesign(saved.id);
            const api = dockOf(workspace);
            if (api) openArchitecture(api);
            if (text) sendText(text);
          }}
        />
      )}
      {picker}
    </>
  );
}
