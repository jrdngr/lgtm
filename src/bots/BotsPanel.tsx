import { useState } from "react";
import type { BotOptions, BotRun } from "../bindings";
import { commands } from "../ipc";
import { useCurrentReview } from "../review/current";
import { useAgentsOff } from "../tools/agents";
import { SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { Installer } from "./Installer";
import { PersonaList } from "./PersonaList";
import { usePersonas } from "./personas";
import { RunList } from "./RunList";
import "./bots.css";

const STORE_KEY = "lgtm.bots";

interface Picks {
  /** Personas left unpicked, so new ones start picked. */
  unpicked: string[];
  options: BotOptions;
}

const DEFAULT_PICKS: Picks = {
  unpicked: [],
  options: { synthesize: true, crossReview: false, adversarial: false, minimal: false },
};

function loadPicks(): Picks {
  try {
    return { ...DEFAULT_PICKS, ...JSON.parse(localStorage.getItem(STORE_KEY) ?? "{}") };
  } catch {
    return DEFAULT_PICKS;
  }
}

const OPTION_LABELS: [keyof BotOptions, string, string][] = [
  ["synthesize", "Synthesis", "Merge every persona's findings into one set"],
  ["crossReview", "Cross-review", "Personas challenge each other's findings before synthesis"],
  ["adversarial", "Adversarial", "Cross-review argues against every medium and higher finding"],
  ["minimal", "Minimal", "Only the findings that matter to what the change is for"],
];

/** Runs personas on the current review, and lists the workspace's runs. The side panel mounts
 * it each time it's shown, so persona files are read again then. */
export function BotsPanel({ onOpen }: { onOpen(run: BotRun): void }) {
  return (
    <div className="side-split">
      <SectionHeader title="Reviewers" />
      <div className="side-split-body">
        <Personas onOpen={onOpen} />
      </div>
    </div>
  );
}

function Personas({ onOpen }: { onOpen(run: BotRun): void }) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const agentsOff = useAgentsOff();
  const { personas, error: loadError, reload } = usePersonas(workspace);
  const [picks, setPicksState] = useState(loadPicks);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string>();
  const [starting, setStarting] = useState(false);
  const { unpicked, options } = picks;

  function setPicks(next: Picks) {
    setPicksState(next);
    localStorage.setItem(STORE_KEY, JSON.stringify(next));
  }

  if (!personas) {
    return loadError ? (
      <div className="bots">
        <div className="bots-error">Couldn't read personas: {loadError}</div>
      </div>
    ) : null;
  }
  const none = personas.personas.length === 0 && personas.broken.length === 0;
  if (installing || none) {
    return (
      <div className="bots">
        {none && <div className="bots-empty">No personas yet. Install the defaults to start.</div>}
        <Installer
          onDone={() => {
            setInstalling(false);
            reload();
          }}
        />
      </div>
    );
  }

  const picked = personas.personas.map((p) => p.name).filter((n) => !unpicked.includes(n));
  const toggle = (name: string) =>
    setPicks({
      ...picks,
      unpicked: unpicked.includes(name) ? unpicked.filter((n) => n !== name) : [...unpicked, name],
    });
  const setOption = (key: keyof BotOptions, on: boolean) =>
    setPicks({ ...picks, options: { ...options, [key]: on } });

  async function start() {
    if (!review) return;
    setStarting(true);
    const r = await commands.startBots(workspace, review.revset, picked, options);
    setStarting(false);
    if (r.status === "error") return setError(r.error);
    setError(undefined);
    onOpen(r.data);
  }

  const lone = picked.length < 2;
  return (
    <div className="bots">
      <PersonaList personas={personas} picked={picked} onToggle={toggle} onChanged={reload} />
      <div className="bots-section">Reports</div>
      <div className="bots-options">
        {OPTION_LABELS.map(([key, label, title]) => {
          const cross = key === "crossReview" || key === "adversarial";
          return (
            <label
              key={key}
              title={cross && lone ? "Needs two or more personas" : title}
              className={key === "adversarial" ? "bots-sub" : undefined}
              hidden={key === "adversarial" && !options.crossReview}
            >
              <input
                type="checkbox"
                disabled={cross && lone}
                checked={options[key] && !(cross && lone)}
                onChange={(e) => setOption(key, e.target.checked)}
              />
              {label}
            </label>
          );
        })}
      </div>
      <div className="bots-actions">
        <button
          type="button"
          disabled={agentsOff || !review || picked.length === 0 || starting}
          onClick={start}
          title="Run each persona headless with claude -p"
        >
          {starting
            ? "Starting…"
            : `Run ${picked.length} ${picked.length === 1 ? "persona" : "personas"}`}
        </button>
        <span className="bots-target">
          {agentsOff ? (
            "Needs claude, which isn't installed or is too old"
          ) : review ? (
            <>
              on <code>{review.revset}</code>
            </>
          ) : (
            "Open a review to run personas on it"
          )}
        </span>
      </div>
      {error && <div className="bots-error">{error}</div>}
      <RunList workspace={workspace} onOpen={onOpen} onError={setError} />
      <div className="bots-footer">
        <button type="button" onClick={() => setInstalling(true)}>
          Install default personas…
        </button>
      </div>
    </div>
  );
}
