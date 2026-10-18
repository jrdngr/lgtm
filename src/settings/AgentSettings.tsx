import { useEffect, useState } from "react";
import type { ClaudeCommand, Settings } from "../bindings";
import { commands } from "../ipc";
import { Busy } from "../ui/Busy";
import { IconButton } from "../ui/icons";
import { Row, Section } from "./parts";
import { saveSettings } from "./store";

/** A text field that saves when it loses focus or Enter is pressed. */
function SavedInput({
  id,
  value,
  placeholder,
  onSave,
}: {
  id: string;
  value: string;
  placeholder?: string;
  onSave(value: string): Promise<string | undefined>;
}) {
  const [draft, setDraft] = useState(value);
  const [error, setError] = useState<string>();
  useEffect(() => setDraft(value), [value]);

  async function save() {
    if (draft === value) return;
    setError(await onSave(draft));
  }

  return (
    <>
      <input
        id={id}
        className="settings-input"
        value={draft}
        placeholder={placeholder}
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
        onChange={(e) => setDraft(e.target.value)}
        onBlur={save}
        onKeyDown={(e) => {
          if (e.key === "Enter") save();
          if (e.key === "Escape") setDraft(value);
        }}
      />
      {error && <div className="error">{error}</div>}
    </>
  );
}

/** How the Claude command resolves, with a refresh that asks the login shell again. */
function Resolved({ setting }: { setting: string }) {
  const [cmd, setCmd] = useState<ClaudeCommand>();
  const [busy, setBusy] = useState(false);

  async function load(refresh: boolean) {
    setBusy(true);
    const r = await commands.claudeCommand(refresh);
    setBusy(false);
    if (r.status === "ok") setCmd(r.data);
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: reload when the setting changes
  useEffect(() => {
    load(false);
  }, [setting]);

  return (
    <div className="settings-resolved">
      {!cmd ? (
        <Busy className="faint">Resolving…</Busy>
      ) : cmd.error ? (
        <span className="error">{cmd.error}</span>
      ) : cmd.viaShell ? (
        <span>
          A shell function, so each run goes through your login shell: <code>{cmd.resolved}</code>
        </span>
      ) : cmd.resolved === cmd.setting ? (
        <span>
          Runs <code>{cmd.resolved}</code>
        </span>
      ) : (
        <span>
          <code>{cmd.setting}</code> → <code>{cmd.resolved}</code>
        </span>
      )}
      <IconButton
        icon="sync"
        label="Read your login shell again"
        onClick={() => load(true)}
        disabled={busy}
      />
    </div>
  );
}

/** The Claude command. */
export function AgentSettings({ settings }: { settings: Settings }) {
  return (
    <Section id="claude" title="Claude Code">
      <Row label="Claude command" htmlFor="settings-claude-terminal">
        <SavedInput
          id="settings-claude-terminal"
          value={settings.claudeTerminal}
          placeholder="claude"
          onSave={(c) => saveSettings({ claudeTerminal: c.trim() || "claude" })}
        />
        <Resolved setting={settings.claudeTerminal} />
      </Row>
    </Section>
  );
}
