import { useEffect, useState } from "react";
import type { Settings } from "../bindings";
import { Row, Section } from "./SettingsPanel";
import { saveSettings } from "./store";

const MIN = 9;
const MAX = 24;

export function TerminalSettings({ settings }: { settings: Settings }) {
  const size = String(settings.terminalFontSize);
  // What's typed, which may be on its way to a valid size.
  const [draft, setDraft] = useState(size);
  const [error, setError] = useState<string>();
  useEffect(() => setDraft(size), [size]);

  async function change(text: string) {
    setDraft(text);
    const n = Number(text);
    if (text === size || !Number.isInteger(n) || n < MIN || n > MAX) return;
    setError(await saveSettings({ terminalFontSize: n }));
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
          value={draft}
          onChange={(e) => change(e.target.value)}
          onBlur={() => setDraft(size)}
        />
        {error && <div className="error">{error}</div>}
      </Row>
    </Section>
  );
}
