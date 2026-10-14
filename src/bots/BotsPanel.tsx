import { useState } from "react";
import type { BotOptions, BotRun, Personas } from "../bindings";
import { commands } from "../ipc";
import { useCurrentReview } from "../review/current";
import { useAgentsOff } from "../tools/agents";
import { Busy } from "../ui/Busy";
import { GrowingText } from "../ui/GrowingText";
import { Icon, IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { Installer } from "./Installer";
import { NewPersona } from "./NewPersona";
import { PersonaList } from "./PersonaList";
import { usePersonas } from "./personas";
import { ReportOptions } from "./ReportOptions";
import { RunList } from "./RunList";
import { seedRun } from "./runs";
import "./bots.css";

const STORE_KEY = "lgtm.bots";

interface Picks {
  /** Personas left unpicked, so new ones start picked. */
  unpicked: string[];
  /** Starred personas, which Favorites picks. */
  favorites: string[];
  options: BotOptions;
}

const DEFAULT_PICKS: Picks = {
  unpicked: [],
  favorites: [],
  options: { synthesize: true, crossReview: false, adversarial: false, minimal: false },
};

function loadPicks(): Picks {
  try {
    return { ...DEFAULT_PICKS, ...JSON.parse(localStorage.getItem(STORE_KEY) ?? "{}") };
  } catch {
    return DEFAULT_PICKS;
  }
}

/** Runs personas on the current review, and lists the workspace's runs. The side panel mounts
 * it each time it's shown, so persona files are read again then. */
export function BotsPanel({ onOpen }: { onOpen(run: BotRun): void }) {
  const loaded = usePersonas();
  const [adding, setAdding] = useState(false);
  const [installing, setInstalling] = useState(false);
  const none = loaded.personas?.personas.length === 0 && loaded.personas.broken.length === 0;

  let body: React.ReactNode;
  if (!loaded.personas) {
    body = null;
  } else if (installing || none) {
    body = (
      <>
        {none && <div className="bots-empty">No personas yet. Install the defaults to start.</div>}
        <Installer
          onDone={() => {
            setInstalling(false);
            loaded.reload();
          }}
        />
      </>
    );
  } else {
    body = <Setup personas={loaded.personas} onOpen={onOpen} onChanged={loaded.reload} />;
  }

  return (
    <div className="side-split">
      <SectionHeader title="Reviewers">
        <IconButton icon="plus" label="New persona" onClick={() => setAdding(true)} />
        <Menu
          items={[{ label: "Install default personas…", onSelect: () => setInstalling(true) }]}
        />
      </SectionHeader>
      <div className="side-split-body bots">
        {adding && (
          <NewPersona
            onClose={() => setAdding(false)}
            onCreated={() => {
              setAdding(false);
              loaded.reload();
            }}
          />
        )}
        {body}
      </div>
    </div>
  );
}

interface SetupProps {
  personas: Personas;
  onOpen(run: BotRun): void;
  onChanged(): void;
}

/** Personas and report options to pick, the Run button, and the runs so far. */
function Setup({ personas, onOpen, onChanged }: SetupProps) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const agentsOff = useAgentsOff();
  const [picks, setPicksState] = useState(loadPicks);
  const [error, setError] = useState<string>();
  const [starting, setStarting] = useState(false);
  const [context, setContext] = useState("");
  const { unpicked, favorites, options } = picks;

  function setPicks(next: Picks) {
    setPicksState(next);
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify(next));
    } catch {}
  }

  const names = personas.personas.map((p) => p.name);
  const picked = names.filter((n) => !unpicked.includes(n));
  const flip = (list: string[], name: string) =>
    list.includes(name) ? list.filter((n) => n !== name) : [...list, name];
  const toggle = (name: string) => setPicks({ ...picks, unpicked: flip(unpicked, name) });
  const star = (name: string) => setPicks({ ...picks, favorites: flip(favorites, name) });
  const pickOnly = (keep: (name: string) => boolean) =>
    setPicks({ ...picks, unpicked: names.filter((n) => !keep(n)) });
  const anyFavorite = names.some((n) => favorites.includes(n));

  async function start() {
    if (!review) return;
    setStarting(true);
    const r = await commands.startBots(workspace, review.revset, picked, options, context);
    setStarting(false);
    if (r.status === "error") return setError(r.error);
    setError(undefined);
    seedRun(r.data);
    onOpen(r.data);
  }

  const n = picked.length;
  // Why Run can't start, in place of what it runs on.
  const blocked = agentsOff
    ? "needs claude, which isn't installed or is too old"
    : !review
      ? "open a review first"
      : n === 0
        ? "pick a reviewer"
        : undefined;

  return (
    <>
      <div className="bots-section">
        <span className="section-meta">
          {n} of {names.length} picked
        </span>
        <span className="spacer" />
        <span className="bots-links">
          <button type="button" onClick={() => pickOnly(() => true)}>
            All
          </button>
          <i>·</i>
          <button type="button" onClick={() => pickOnly(() => false)}>
            None
          </button>
          <i>·</i>
          <button
            type="button"
            disabled={!anyFavorite}
            title={anyFavorite ? "Pick just the starred reviewers" : "Star a reviewer first"}
            onClick={() => pickOnly((n) => favorites.includes(n))}
          >
            <Icon name="star" size={10} />
            Favorites
          </button>
        </span>
      </div>
      <PersonaList
        personas={personas}
        picked={picked}
        favorites={favorites}
        onToggle={toggle}
        onStar={star}
        onChanged={onChanged}
      />
      <div className="bots-section">
        <span className="bots-label">Report</span>
      </div>
      <ReportOptions
        options={options}
        lone={n < 2}
        onChange={(o) => setPicks({ ...picks, options: o })}
      />
      <div className="bots-section">
        <span className="bots-label">Context</span>
      </div>
      <GrowingText
        className="bots-context"
        value={context}
        maxLines={5}
        placeholder="What the reviewers should know about this change"
        onChange={(e) => setContext(e.target.value)}
      />
      <button
        type="button"
        className="primary bots-run-button"
        disabled={!!blocked || starting}
        onClick={start}
        title="Run each persona headless with claude -p"
      >
        {starting ? (
          <Busy>Starting…</Busy>
        ) : n ? (
          `Run ${n} ${n === 1 ? "reviewer" : "reviewers"}`
        ) : (
          "Run reviewers"
        )}
        <small>{blocked ?? <>on {review?.revset}</>}</small>
      </button>
      {error && <div className="bots-error">{error}</div>}
      <RunList workspace={workspace} onOpen={onOpen} onError={setError} />
    </>
  );
}
