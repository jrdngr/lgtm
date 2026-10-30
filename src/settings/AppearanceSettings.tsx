import type { Fonts, Settings } from "../bindings";
import { Row, Section } from "./parts";
import { saveSettings } from "./store";

const FONTS: Record<Fonts, string> = { system: "System", geist: "Geist" };

export function AppearanceSettings({ settings }: { settings: Settings }) {
  return (
    <Section id="appearance" title="Appearance">
      <Row label="Fonts" htmlFor="settings-fonts">
        <select
          id="settings-fonts"
          className="settings-input"
          value={settings.fonts}
          onChange={(e) => saveSettings({ fonts: e.target.value as Fonts })}
        >
          {Object.entries(FONTS).map(([value, name]) => (
            <option key={value} value={value}>
              {name}
            </option>
          ))}
        </select>
      </Row>
    </Section>
  );
}
