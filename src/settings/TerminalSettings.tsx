import { useState } from "react";
import type { Settings } from "../bindings";
import { Row, Section } from "./SettingsPanel";
import { saveSettings } from "./store";

const MIN = 9;
const MAX = 24;

export function TerminalSettings({ settings }: { settings: Settings }) {
  const [error, setError] = useState<string>();

  async function save(size: number) {
    if (!Number.isInteger(size) || size < MIN || size > MAX) return;
    setError(await saveSettings({ terminalFontSize: size }));
  }

  return (
    <Section id="terminal" title="Terminal">
      <Row label="Font size" help={`${MIN} to ${MAX} points`} htmlFor="settings-font-size">
        <input
          id="settings-font-size"
          className="settings-input settings-number"
          type="number"
          min={MIN}
          max={MAX}
          value={settings.terminalFontSize}
          onChange={(e) => save(e.target.valueAsNumber)}
        />
        {error && <div className="error">{error}</div>}
      </Row>
    </Section>
  );
}
