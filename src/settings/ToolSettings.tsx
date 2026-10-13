import { useCallback, useEffect, useState } from "react";
import type { McpStatus, ToolStatus } from "../bindings";
import { commands, errorText, unwrap } from "../ipc";
import { IconButton } from "../ui/icons";
import { Section } from "./SettingsPanel";
import { useSettings } from "./store";

const MCP_TEXT: Record<McpStatus["setup"], string> = {
  registered: "Registered to this app",
  elsewhere: "Registered to another lgtm build",
  missing: "Not registered",
};

/** Claude Code's `lgtm` MCP server: its registration and a button to (re)register it. */
function Mcp() {
  const [status, setStatus] = useState<McpStatus>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const load = useCallback(() => {
    unwrap(commands.mcpStatus()).then(setStatus, (e) => setError(errorText(e)));
  }, []);
  useEffect(load, [load]);

  async function register() {
    setBusy(true);
    setError(undefined);
    try {
      await unwrap(commands.registerMcp());
      load();
    } catch (e) {
      setError(errorText(e));
    }
    setBusy(false);
  }

  return (
    <tr>
      <th scope="row">MCP</th>
      <td colSpan={2}>
        <span className={status?.setup === "registered" ? "settings-ok" : "settings-warn"}>
          {status ? MCP_TEXT[status.setup] : "…"}
        </span>
        {status && (
          <div className="settings-help">
            <code>{status.command}</code>
          </div>
        )}
        {error && <div className="error">{error}</div>}
      </td>
      <td className="settings-tool-action">
        {status && status.setup !== "registered" && (
          <button type="button" onClick={register} disabled={busy}>
            {busy ? "Registering…" : "Register"}
          </button>
        )}
      </td>
    </tr>
  );
}

/** Where jj, claude and gh were found and their versions, rechecked after the Claude command
 * changes. */
export function ToolSettings() {
  const claude = useSettings()?.claude;
  const [tools, setTools] = useState<ToolStatus[]>();
  const [busy, setBusy] = useState(false);

  const check = useCallback(async () => {
    setBusy(true);
    setTools(await commands.toolStatus());
    setBusy(false);
  }, []);
  // biome-ignore lint/correctness/useExhaustiveDependencies: recheck when the command changes
  useEffect(() => {
    check();
  }, [check, claude]);

  return (
    <Section id="tools" title="Tools">
      <div className="settings-tools-head">
        <span className="settings-help">What LGTM runs, as found on your login shell's PATH.</span>
        <IconButton icon="sync" label="Check again" onClick={check} disabled={busy} />
      </div>
      <table className="settings-tools">
        <tbody>
          {(tools ?? []).map((t) => (
            <tr key={t.tool}>
              <th scope="row">{t.tool}</th>
              <td className={t.problem ? "settings-warn" : "settings-ok"}>
                {t.version ?? "not found"}
              </td>
              <td className="settings-path">{t.location ?? ""}</td>
              <td className="settings-tool-action">
                {t.problem && (
                  <span className="settings-help">
                    {t.problem.found ? `Needs ${t.problem.minimum}: ` : ""}
                    <code>{t.problem.fix}</code>
                  </span>
                )}
              </td>
            </tr>
          ))}
          <Mcp />
        </tbody>
      </table>
    </Section>
  );
}
