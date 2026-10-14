import type { IDockviewPanelProps } from "dockview-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { commands } from "../ipc";
import { AgentSettings } from "./AgentSettings";
import type { SettingsParams } from "./open";
import { PromptSettings } from "./PromptSettings";
import { useSettings } from "./store";
import { TerminalSettings } from "./TerminalSettings";
import { ToolSettings } from "./ToolSettings";
import "./settings.css";

/** A titled block of settings, which the tab can scroll to by `id`. */
export function Section({
  id,
  title,
  children,
}: {
  id: string;
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="settings-section" data-section={id} aria-labelledby={`settings-${id}`}>
      <h2 id={`settings-${id}`}>{title}</h2>
      {children}
    </section>
  );
}

/** One setting: its label and help on the left, its control on the right. */
export function Row({
  label,
  help,
  htmlFor,
  children,
}: {
  label: string;
  help?: ReactNode;
  htmlFor?: string;
  children: ReactNode;
}) {
  return (
    <div className="settings-row">
      <div className="settings-label">
        <label htmlFor={htmlFor}>{label}</label>
        {help && <div className="settings-help">{help}</div>}
      </div>
      <div className="settings-control">{children}</div>
    </div>
  );
}

/** App settings, saved to `~/.config/lgtm/settings.toml`, and the repo's prompt files. */
export function SettingsPanel({ params }: IDockviewPanelProps<SettingsParams>) {
  const settings = useSettings();
  const body = useRef<HTMLDivElement>(null);
  const { section, nonce } = params;
  const [loadError, setLoadError] = useState<string | null>(null);

  // A save replaces the file that failed to load, so each change asks again.
  useEffect(() => {
    if (settings) commands.settingsLoadError().then(setLoadError);
  }, [settings]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: each open (nonce) scrolls again
  useEffect(() => {
    if (!section || !settings) return;
    body.current?.querySelector(`[data-section="${section}"]`)?.scrollIntoView({ block: "start" });
  }, [section, nonce, !settings]);

  return (
    <div className="settings" ref={body}>
      <div className="settings-page">
        <h1>Settings</h1>
        {loadError && (
          <p className="settings-warn">
            settings.toml couldn't be read, so the defaults are in use: {loadError}. The next change
            here backs the file up to settings.toml.bak and replaces it.
          </p>
        )}
        {settings ? (
          <>
            <AgentSettings settings={settings} />
            <TerminalSettings settings={settings} />
            <ToolSettings />
            <PromptSettings />
          </>
        ) : (
          <div className="placeholder">Loading…</div>
        )}
      </div>
    </div>
  );
}
