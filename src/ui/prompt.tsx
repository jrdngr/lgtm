import { message } from "@tauri-apps/plugin-dialog";
import { type KeyboardEvent, useEffect, useRef, useSyncExternalStore } from "react";

interface Ask {
  title: string;
  value: string;
  /** The button's label, like Rename. */
  action: string;
  /** A description box: ↵ adds a line and ⌘↵ submits. */
  multiline?: boolean;
  /** Submitting it empty gives "" rather than undefined. */
  allowEmpty?: boolean;
  /** Where focus was, to go back to after. */
  from: Element | null;
  /** Tells one ask's box from the next, so a new one starts with its own text. */
  id: number;
  done(value: string | undefined): void;
}

let asks = 0;

let current: Ask | undefined;
const listeners = new Set<() => void>();
const set = (a: Ask | undefined) => {
  current = a;
  for (const l of listeners) l();
};

/** Asks for one line of text, or several with `multiline`. Undefined when cancelled, or left
 * empty without `allowEmpty`. One question at a time: a new one cancels the last. */
export function promptText(
  title: string,
  opts: { value?: string; action?: string; multiline?: boolean; allowEmpty?: boolean } = {},
): Promise<string | undefined> {
  current?.done(undefined);
  return new Promise((done) =>
    set({
      title,
      value: opts.value ?? "",
      action: opts.action ?? "OK",
      multiline: opts.multiline,
      allowEmpty: opts.allowEmpty,
      from: document.activeElement,
      id: ++asks,
      done,
    }),
  );
}

/** A native yes/no for a destructive or risky action, `action` and `cancel` naming its
 * buttons. */
export async function confirmAction(
  text: string,
  action: string,
  cancel = "Cancel",
  title?: string,
): Promise<boolean> {
  const choice = await message(text, {
    title,
    kind: "warning",
    buttons: { ok: action, cancel },
  });
  return choice === action;
}

/** Where `promptText` shows its box, once per window. */
export function PromptHost() {
  const ask = useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => current,
  );
  const ref = useRef<HTMLInputElement & HTMLTextAreaElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.focus();
    // A file name is selected up to its extension, as Finder does.
    const dot = ask?.multiline ? -1 : el.value.lastIndexOf(".");
    el.setSelectionRange(0, dot > 0 ? dot : el.value.length);
  }, [ask]);
  if (!ask) return null;

  const finish = (value: string | undefined) => {
    set(undefined);
    if (ask.from instanceof HTMLElement) ask.from.focus();
    const text = value?.trim();
    ask.done(text || (text === "" && ask.allowEmpty ? "" : undefined));
  };
  const submit = () => finish(ref.current?.value ?? "");
  const keys = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape") finish(undefined);
    else if (e.key === "Enter" && (!ask.multiline || e.metaKey)) {
      e.preventDefault();
      submit();
    }
  };
  return (
    <div
      className="prompt-backdrop"
      onPointerDown={(e) => e.target === e.currentTarget && finish(undefined)}
    >
      <form
        className="prompt"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <label htmlFor="prompt-text">{ask.title}</label>
        {ask.multiline ? (
          <textarea
            key={ask.id}
            id="prompt-text"
            ref={ref}
            defaultValue={ask.value}
            rows={6}
            onKeyDown={keys}
          />
        ) : (
          <input
            key={ask.id}
            id="prompt-text"
            ref={ref}
            defaultValue={ask.value}
            onKeyDown={keys}
            spellCheck={false}
          />
        )}
        <div className="prompt-buttons">
          <button type="button" className="quiet" onClick={() => finish(undefined)}>
            Cancel
          </button>
          <button type="submit" className="primary">
            {ask.action}
          </button>
        </div>
      </form>
    </div>
  );
}
