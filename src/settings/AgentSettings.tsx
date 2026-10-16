import { useEffect, useState } from "react";
import type { ClaudeCommand, Models, Settings } from "../bindings";
import { commands } from "../ipc";
import { Busy } from "../ui/Busy";
import { IconButton } from "../ui/icons";
import { Row, Section } from "./SettingsPanel";
import { saveSettings } from "./store";

const MODELS: [keyof Models, string, string][] = [
  ["ask", "Fixes", "Fix on intents"],
  ["walkthrough", "Assess", "Assessing the architecture"],
  ["replies", "Replies", "Triage and replies on PR threads"],
  ["qa", "Q&A", "Questions, plans and Apply in Q&A sessions"],
];

/** A text field that saves when it loses focus or Enter is pressed. */
function SavedInput({
  id,
  value,
  placeholder,
  list,
  onSave,
}: {
  id: string;
  value: string;
  placeholder?: string;
  list?: string;
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
        list={list}
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

/** The Claude command and the model for each kind of agent run. */
export function AgentSettings({ settings }: { settings: Settings }) {
  const saveModel = (key: keyof Models) => (value: string) =>
    saveSettings({ models: { ...settings.models, [key]: value.trim() } });

  return (
    <>
      <Section id="claude" title="Claude Code">
        <Row
          label="Claude command"
          htmlFor="settings-claude"
          help={
            <>
              What runs Claude Code for every agent: a command, or an alias or function from your
              login shell like <code>cc</code>. LGTM adds its own arguments after it.
            </>
          }
        >
          <SavedInput
            id="settings-claude"
            value={settings.claude}
            placeholder="claude"
            onSave={(claude) => saveSettings({ claude: claude.trim() || "claude" })}
          />
          <Resolved setting={settings.claude} />
        </Row>
      </Section>
      <Section id="models" title="Models">
        <p className="settings-help">
          Passed to <code>--model</code>. Leave one empty for Claude Code's default.
        </p>
        <datalist id="settings-model-names">
          <option value="opus" />
          <option value="sonnet" />
          <option value="haiku" />
        </datalist>
        {MODELS.map(([key, label, help]) => (
          <Row key={key} label={label} help={help} htmlFor={`settings-model-${key}`}>
            <SavedInput
              id={`settings-model-${key}`}
              value={settings.models[key]}
              placeholder="Default"
              list="settings-model-names"
              onSave={saveModel(key)}
            />
          </Row>
        ))}
      </Section>
    </>
  );
}
