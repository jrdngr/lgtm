import type { IDisposable, Terminal } from "@xterm/xterm";
import { openUrl } from "../services/pathMenu";
import { heldLink } from "./termLinks";

const URL = /\bhttps?:\/\/[^\s"'<>`]+/g;

/** The web addresses in `text`, by character, without the punctuation that ends a sentence. */
export function urlsIn(text: string): { from: number; to: number; url: string }[] {
  return [...text.matchAll(URL)].map((m) => {
    const url = m[0].replace(/[.,;:!?)\]}]+$/, "");
    return { from: m.index, to: m.index + url.length, url };
  });
}

/** ⌘-clicking a web address in the terminal's output opens it in the browser. */
export function urlLinks(term: Terminal): IDisposable {
  return term.registerLinkProvider({
    provideLinks(y, callback) {
      const text = term.buffer.active.getLine(y - 1)?.translateToString(true) ?? "";
      const found = urlsIn(text);
      if (!found.length) return callback(undefined);
      callback(
        found.map((f) =>
          heldLink({ start: { x: f.from + 1, y }, end: { x: f.to, y } }, f.url, () =>
            openUrl(f.url),
          ),
        ),
      );
    },
  });
}
