import {
  closeSearchPanel,
  findNext,
  findPrevious,
  openSearchPanel,
  SearchQuery,
  setSearchQuery,
} from "@codemirror/search";
import type { EditorView, ViewUpdate } from "@codemirror/view";
import { useEffect, useState } from "react";
import type { FindState } from "./FindBar";

/** Counted matches stop here, so a huge file stays quick. */
const MAX = 9999;

const query = (s: FindState) => new SearchQuery({ search: s.query, caseSensitive: s.matchCase });

/** The query's matches in `view`, and which one the selection is on. */
function count(view: EditorView, q: SearchQuery) {
  if (!q.valid) return { at: 0, total: 0 };
  const sel = view.state.selection.main;
  const cursor = q.getCursor(view.state);
  let total = 0;
  let at = 0;
  for (let m = cursor.next(); !m.done && total < MAX; m = cursor.next()) {
    if (m.value.from === sel.from && m.value.to === sel.to) at = total;
    total++;
  }
  return { at, total };
}

/** Find in the editor, through CodeMirror's search, which highlights the matches. Its own
 * panel stays empty: the find bar stands in for it. */
export function useCmFind(view: EditorView | undefined) {
  const [open, setOpen] = useState(false);
  const [focusKey, setFocusKey] = useState(0);
  const [state, setState] = useState<FindState>({ query: "", matchCase: false });
  const [found, setFound] = useState({ at: 0, total: 0 });

  useEffect(() => {
    if (!view) return;
    if (!open) {
      closeSearchPanel(view);
      return;
    }
    const q = query(state);
    openSearchPanel(view);
    view.dispatch({ effects: setSearchQuery.of(q) });
    // Typing moves to the first match from where the selection starts.
    if (q.valid) {
      const from = view.state.selection.main.from;
      view.dispatch({ selection: { anchor: from } });
      findNext(view);
    }
    setFound(count(view, q));
  }, [view, open, state]);

  return {
    open,
    show() {
      const picked = view?.state.sliceDoc(
        view.state.selection.main.from,
        view.state.selection.main.to,
      );
      if (!open && picked && !picked.includes("\n")) setState((s) => ({ ...s, query: picked }));
      setOpen(true);
      setFocusKey((k) => k + 1);
    },
    /** Keeps the count current as the text or selection changes. */
    changed(u: ViewUpdate) {
      if (open && (u.docChanged || u.selectionSet)) setFound(count(u.view, query(state)));
    },
    bar: {
      ...state,
      ...found,
      focusKey,
      onChange: setState,
      onStep(back: boolean) {
        if (!view) return;
        (back ? findPrevious : findNext)(view);
        setFound(count(view, query(state)));
      },
      onClose() {
        setOpen(false);
        view?.focus();
      },
    },
  };
}
