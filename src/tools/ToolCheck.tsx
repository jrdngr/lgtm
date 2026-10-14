import { type ReactNode, useEffect, useState } from "react";
import type { ToolProblem } from "../bindings";
import { commands } from "../ipc";
import { setToolProblems } from "./agents";
import "./tools.css";

const NEEDED_BY: Record<string, string> = {
  claude: "Reviewers and comment agents",
  gh: "GitHub PRs",
};

function describe(p: ToolProblem) {
  const what = p.found
    ? `${p.tool} ${p.found} is older than ${p.minimum}`
    : `${p.tool} isn't installed`;
  const needs = p.required ? "LGTM needs it." : `${NEEDED_BY[p.tool] ?? "Some features"} need it.`;
  return `${what}. ${needs}`;
}

/** Checks for jj, claude and gh before showing the app. Without jj nothing works, so that
 * can't be dismissed. Without claude the app runs with its agent features off, and without gh
 * with its GitHub features off. */
export function ToolCheck({ children }: { children: ReactNode }) {
  const [problems, setProblems] = useState<ToolProblem[]>();
  const [checking, setChecking] = useState(false);
  const [dismissed, setDismissed] = useState(false);

  async function check() {
    setChecking(true);
    const found = await commands.checkTools();
    setChecking(false);
    setProblems(found);
    setToolProblems(found.map((p) => p.tool));
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: check once at startup
  useEffect(() => {
    check();
  }, []);

  if (!problems) return null;
  if (problems.length === 0 || dismissed) return <>{children}</>;
  const blocking = problems.some((p) => p.required);
  return (
    <div className="tool-check">
      <div className="tool-check-dialog" role="alertdialog" aria-labelledby="tool-check-title">
        <h2 id="tool-check-title">
          {blocking ? "LGTM can't start yet" : "Some tools are missing"}
        </h2>
        <ul>
          {problems.map((p) => (
            <li key={p.tool}>
              <div>{describe(p)}</div>
              <code>{p.fix}</code>
            </li>
          ))}
        </ul>
        <div className="tool-check-actions">
          <button type="button" onClick={check} disabled={checking}>
            {checking ? "Checking…" : "Check again"}
          </button>
          {!blocking && (
            <button type="button" onClick={() => setDismissed(true)}>
              Continue without {problems.map((p) => p.tool).join(" and ")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
