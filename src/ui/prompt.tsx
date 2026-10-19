import { message } from "@tauri-apps/plugin-dialog";
import {
  type KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useRef,
  useSyncExternalStore,
} from "react";

interface Ask {
  title: string;
  value: string;
  /** The button's label, like Rename. */
  action: string;
  /** A description box: ↵ adds a line and ⌘↵ submits. */
  multiline?: boolean;
  /** Submitting it empty gives "" rather than undefined. */
  allowEmpty?: boolean;
  /** Text shown whole and read-only in place of a box, as `confirmShown` asks. */
  shown?: string;
  cancel?: string;
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

/** Whether `confirmShown` is up. It's answered only by its buttons or Escape, so a new ask is
 * refused while it is, rather than cancelling it. */
const held = () => current?.shown !== undefined;

/** Answers `ask` and puts focus back where it was. */
function answer(ask: Ask, value: string | undefined) {
  set(undefined);
  if (ask.from instanceof HTMLElement) ask.from.focus();
  const text = value?.trim();
  ask.done(text || (text === "" && ask.allowEmpty ? "" : undefined));
}

/** Whether a prompt takes `e`, so app shortcuts leave it alone: it was typed in the prompt, or
 * one is up and focus is elsewhere, when the key is swallowed and Escape cancels the prompt. */
export function promptKeeps(e: KeyboardEvent): boolean {
  if ((e.target as Element).closest?.(".prompt")) return true;
  if (!current) return false;
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") answer(current, undefined);
  return true;
}

/** Asks for one line of text, or several with `multiline`. Undefined when cancelled, or left
 * empty without `allowEmpty`. One question at a time: a new one cancels the last. */
export function promptText(
  title: string,
  opts: { value?: string; action?: string; multiline?: boolean; allowEmpty?: boolean } = {},
): Promise<string | undefined> {
  if (held()) return Promise.resolve(undefined);
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

/** Shows `text` whole in a scrolling block and asks whether to go ahead, `action` and
 * `cancel` naming the buttons. In the window, so a long text can't push the buttons away. */
export async function confirmShown(
  title: string,
  text: string,
  action: string,
  cancel = "Cancel",
): Promise<boolean> {
  if (held()) return false;
  current?.done(undefined);
  const value = await new Promise<string | undefined>((done) =>
    set({
      title,
      value: "",
      action,
      cancel,
      allowEmpty: true,
      shown: text,
      from: document.activeElement,
      id: ++asks,
      done,
    }),
  );
  return value !== undefined;
}

/** Shows `text` as `confirmShown` does, with only a `button` to close it. */
export async function showText(title: string, text: string, button: string): Promise<void> {
  await confirmShown(title, text, "", button);
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
  const shownRef = useRef<HTMLPreElement>(null);
  useEffect(() => {
    shownRef.current?.focus();
    const el = ref.current;
    if (!el) return;
    el.focus();
    // A file name is selected up to its extension, as Finder does.
    const dot = ask?.multiline ? -1 : el.value.lastIndexOf(".");
    el.setSelectionRange(0, dot > 0 ? dot : el.value.length);
  }, [ask]);
  if (!ask) return null;

  const finish = (value: string | undefined) => answer(ask, value);
  const submit = () => finish(ref.current?.value ?? "");
  const keys = (e: ReactKeyboardEvent) => {
    e.stopPropagation();
    // Enter on a button presses that button.
    const inBox = e.target === ref.current || e.target === shownRef.current;
    if (e.key === "Escape") finish(undefined);
    else if (
      inBox &&
      e.key === "Enter" &&
      ((!ask.multiline && ask.shown === undefined) || e.metaKey)
    ) {
      e.preventDefault();
      submit();
    }
  };
  return (
    <div
      className="prompt-backdrop"
      // Focus stays in the prompt, so its keys keep reaching it.
      onMouseDown={(e) => e.target === e.currentTarget && e.preventDefault()}
      onPointerDown={(e) =>
        e.target === e.currentTarget && ask.shown === undefined && finish(undefined)
      }
    >
      <form
        className={ask.shown === undefined ? "prompt" : "prompt wide"}
        onKeyDown={keys}
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <label htmlFor="prompt-text">{ask.title}</label>
        {ask.shown !== undefined ? (
          <pre key={ask.id} id="prompt-text" ref={shownRef} tabIndex={-1}>
            {ask.shown}
          </pre>
        ) : ask.multiline ? (
          <textarea key={ask.id} id="prompt-text" ref={ref} defaultValue={ask.value} rows={6} />
        ) : (
          <input
            key={ask.id}
            id="prompt-text"
            ref={ref}
            defaultValue={ask.value}
            spellCheck={false}
          />
        )}
        <div className="prompt-buttons">
          <button type="button" className="quiet" onClick={() => finish(undefined)}>
            {ask.cancel ?? "Cancel"}
          </button>
          {ask.action && (
            <button type="submit" className="primary">
              {ask.action}
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
