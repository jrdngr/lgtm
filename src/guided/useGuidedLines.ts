import { useEffect, useMemo, useRef, useState } from "react";
import type { MarkColor, RowRef } from "../bindings";
import { isMac } from "../core/keys";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";
import { colorForKey, type MarkSetup } from "../review/marks";
import { markKey } from "../review/model";
import { rowsRefs } from "../review/sendText";
import { unified } from "../review/snippet";
import type { Comments } from "../review/useComments";
import { clearTextSelection } from "../review/useRowMouse";
import { searchItems } from "../search/menu";
import { wordAtPoint } from "../search/symbolAt";
import { type CtxEntry, copy, openMenu } from "../ui/contextMenu";
import type { Segment } from "./model";

interface Deps {
  workspace: WorkspaceName;
  review: CurrentReview;
  setup: MarkSetup;
  segments: Segment[];
  c: Comments;
  send(text: string): void;
  setError(message: string): void;
}

/** Picking lines in a guided diff and acting on them as the Review tab does: move with j/k,
 * extend with J/K, shift-click or a drag, mark with 1–9 and 0, comment with c or ⇧C, send with ⌘>,
 * copy with ⌘C, open with o, undo marks with ⌘Z, and the same from a row's menu. */
export function useGuidedLines({ workspace, review, setup, segments, c, send, setError }: Deps) {
  const [ends, setEnds] = useState<{ anchor: string; head: string }>();
  /** The rows shown, in order. */
  const order = useMemo(
    () => segments.flatMap((s) => s.rows.map((row) => ({ file: s.file, row }))),
    [segments],
  );
  const at = (key: string | undefined) =>
    key === undefined ? -1 : order.findIndex((r) => markKey(r.file, r.row) === key);
  const a = at(ends?.anchor);
  const h = at(ends?.head);
  const picked: RowRef[] = a < 0 || h < 0 ? [] : order.slice(Math.min(a, h), Math.max(a, h) + 1);
  const pickedKeys = new Set(picked.map((r) => markKey(r.file, r.row)));
  const head = h < 0 ? undefined : order[h];

  const keyOf = (r: RowRef) => markKey(r.file, r.row);
  const pick = (r: RowRef, extend: boolean) =>
    setEnds((e) => ({ anchor: extend && e ? e.anchor : keyOf(r), head: keyOf(r) }));
  const clear = () => setEnds(undefined);
  const dragging = useRef(false);

  useEffect(() => {
    const up = () => {
      dragging.current = false;
    };
    window.addEventListener("mouseup", up);
    return () => window.removeEventListener("mouseup", up);
  }, []);

  /** Picks rows as the Review tab does: code text stays selectable, and a double-click there
   * selects a word; gutters, or a triple click anywhere, pick rows. */
  function onRowDown(r: RowRef, e: React.MouseEvent) {
    if (e.button !== 0) return;
    const inCode = e.target instanceof Element && e.target.closest(".code");
    if (e.detail === 2 && inCode) return;
    if (!inCode || e.detail >= 3) {
      e.preventDefault();
      clearTextSelection();
    }
    if (e.detail >= 3) {
      const seg = segments.find((g) => g.file === r.file && g.rows.includes(r.row));
      if (seg) {
        const last = seg.rows[seg.rows.length - 1];
        setEnds({ anchor: markKey(r.file, seg.rows[0]), head: markKey(r.file, last) });
      }
      return;
    }
    pick(r, e.shiftKey);
    dragging.current = !e.shiftKey;
  }

  const onRowEnter = (r: RowRef) => {
    if (dragging.current) pick(r, true);
  };
  const fail = (e: unknown) => setError(String(e));
  const isChanged = (r: RowRef) => review.files[r.file].rows[r.row].kind !== "context";

  /** `rows`, by file in order. */
  function byFile(rows: RowRef[]): [number, number[]][] {
    const out: [number, number[]][] = [];
    for (const r of rows) {
      const last = out[out.length - 1];
      if (last && last[0] === r.file) last[1].push(r.row);
      else out.push([r.file, [r.row]]);
    }
    return out;
  }

  /** What a comment on `picks` covers: the changed rows of the last file that has any. */
  function commentRows(picks: RowRef[]): [number, number[]] | undefined {
    for (const [file, rows] of byFile(picks).reverse()) {
      const changed = rows.filter((row) => isChanged({ file, row }));
      if (changed.length) return [file, changed];
    }
    return undefined;
  }

  function comment(github: boolean, picks = picked) {
    const target = commentRows(picks);
    if (!target) return setError("Pick changed lines to comment on.");
    const [file, rows] = target;
    c.start({ file, rows, snippet: unified(rows.map((r) => review.files[file].rows[r])), github });
    clear();
  }

  function mark(color: MarkColor | null, picks = picked) {
    const rows = picks.filter(isChanged);
    if (!rows.length) return setError("No changed lines picked.");
    unwrap(commands.markRows(review.id, rows, color, [])).then((s) => {
      review.apply(s, review.id);
      clear();
    }, fail);
  }

  const undo = (back: boolean) =>
    unwrap(back ? commands.undoMarks(review.id) : commands.redoMarks(review.id)).then(
      (s) => review.apply(s, review.id),
      fail,
    );

  function sendPicked(picks = picked) {
    const text = byFile(picks)
      .map(([file, rows]) => rowsRefs(review.files[file], rows))
      .join("\n");
    if (text) send(text);
  }

  function copyPicked(picks = picked) {
    const text = picks.map((r) => review.files[r.file].rows[r.row].text).join("\n");
    if (text) copy(text);
  }

  function openHead() {
    const r = head ?? order[0];
    if (!r) return;
    const row = review.files[r.file].rows[r.row];
    openFile(workspace, review.files[r.file].dto.path, row.newLine ?? undefined);
  }

  function move(dir: 1 | -1, extend: boolean) {
    if (!order.length) return;
    const next = h < 0 ? (dir > 0 ? 0 : order.length - 1) : h + dir;
    const r = order[Math.max(0, Math.min(order.length - 1, next))];
    pick(r, extend);
    document.querySelector(`[data-row="${keyOf(r)}"]`)?.scrollIntoView({ block: "nearest" });
  }

  /** Handles a key for the picked lines. Returns whether it did, so the view's own keys
   * (like Change on `c` with nothing picked) still work. */
  function onKey(e: React.KeyboardEvent): boolean {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod) {
      const key = e.key.toLowerCase();
      if ((e.key === ">" || e.key === ".") && e.shiftKey && picked.length) sendPicked();
      else if (key === "c" && picked.length && window.getSelection()?.isCollapsed !== false)
        copyPicked();
      else if (key === "z") undo(!e.shiftKey);
      else return false;
      return true;
    }
    if (e.altKey) return false;
    const has = picked.length > 0;
    switch (e.key) {
      case "j":
      case "ArrowDown":
        move(1, false);
        return true;
      case "k":
      case "ArrowUp":
        move(-1, false);
        return true;
      case "J":
      case "K":
        move(e.key === "J" ? 1 : -1, true);
        return true;
      case "o":
        openHead();
        return true;
      case "Escape":
        if (!has) return false;
        clear();
        return true;
      case "0":
        if (!has || e.repeat) return false;
        mark(null);
        return true;
      case "c":
      case "C":
        if (!has || !commentRows(picked) || e.repeat) return false;
        if (e.key === "C" && !c.canDraft) return false;
        comment(e.key === "C");
        return true;
    }
    const color = has && !e.repeat ? colorForKey(setup, e.key) : undefined;
    if (!color) return false;
    mark(color);
    return true;
  }

  function menu(picks: RowRef[]): CtxEntry[] {
    const canComment = !!commentRows(picks);
    return [
      { label: "Send to Terminal", keys: "⌘>", onSelect: () => sendPicked(picks) },
      { label: "Copy", keys: "⌘C", onSelect: () => copyPicked(picks) },
      "-",
      canComment && { label: "Comment", keys: "c", onSelect: () => comment(false, picks) },
      canComment &&
        c.canDraft && { label: "Draft", keys: "⇧C", onSelect: () => comment(true, picks) },
      {
        label: "Mark",
        items: [
          ...setup.shown.map((s, i) => ({
            label: setup.label(s.color),
            keys: `${i + 1}`,
            onSelect: () => mark(s.color, picks),
          })),
          "-",
          { label: "Clear", keys: "0", onSelect: () => mark(null, picks) },
        ],
      },
      "-",
      { label: "Open in Editor", keys: "o", onSelect: openHead },
    ];
  }

  /** A row's right-click acts on the picked lines when it's one of them, else on it alone. */
  const onRowMenu = (r: RowRef) => (e: React.MouseEvent) => {
    const picks = pickedKeys.has(keyOf(r)) ? picked : [r];
    const word = wordAtPoint(e.clientX, e.clientY);
    if (picks.length === 1) pick(r, false);
    const path = review?.files[r.file]?.dto.path;
    openMenu(e, [...searchItems(workspace, word, path), ...menu(picks)]);
  };

  return {
    picked,
    isPicked: (file: number, row: number) => pickedKeys.has(markKey(file, row)),
    isHead: (file: number, row: number) => !!head && head.file === file && head.row === row,
    onRowDown,
    onRowEnter,
    onRowMenu,
    onKey,
    clear,
    comment,
    canComment: !!commentRows(picked),
  };
}

export type GuidedLines = ReturnType<typeof useGuidedLines>;
