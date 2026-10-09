// The canvas's keyboard shortcuts (specification 39, §3.3 item 8). They are registered as soon as
// a page is drawn, so a key pressed at once finds them. The keys act only while the canvas has the
// focus, or the page with no control focused: a button, a field, a menu or a dialog keeps its keys
// (§21.3 item 6), and Tab moves the focus on from there. A press on the canvas gives it the keys.

import { useLayoutEffect } from "react";

import type { HmiPageModel } from "../../drawing/model";
import { deleteObjects, Direction, nudge, resizeBy } from "../editing";
import { deletePipes } from "../pipes";
import { canvasOwnsKey, pressTakesKeys } from "./canvasKeys";
import type { EditAction } from "./ContextMenu";

const ARROWS: Record<string, Direction> = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down" };

export interface CanvasShortcuts {
  page: HmiPageModel | undefined;
  paused: boolean;
  selection: string[];
  hiddenObjects: string[];
  commit: (page: HmiPageModel) => void;
  setSelection: (ids: string[]) => void;
  setZoom: (zoom: number) => void;
  fit: () => void;
  zoomToSelection: () => void;
  /** The context menu's actions, current at each key. */
  actRef: { current: (action: EditAction) => void };
}

export function useCanvasShortcuts({
  page,
  paused,
  selection,
  hiddenObjects,
  commit,
  setSelection,
  setZoom,
  fit,
  zoomToSelection,
  actRef,
}: CanvasShortcuts): void {
  useLayoutEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      if (!canvasOwnsKey(event.target)) return;
      if (!page || paused) return;
      if (event.key === "Tab" && page.object.length > 0) {
        event.preventDefault();
        const ids = page.object.map((object) => object.id);
        const at = selection.length > 0 ? ids.indexOf(selection[selection.length - 1]) : -1;
        const next =
          at < 0 ? (event.shiftKey ? ids.length - 1 : 0) : (at + (event.shiftKey ? ids.length - 1 : 1)) % ids.length;
        setSelection([ids[next]]);
      } else if (event.key === "/" && !event.ctrlKey && !event.metaKey) {
        event.preventDefault();
        (document.querySelector('input[name="component-search"]') as HTMLInputElement | null)?.focus();
      } else if (event.key === "Enter" && selection.length > 0) {
        event.preventDefault();
        (
          document.querySelector(".hb-inspector input:not([readonly]), .hb-inspector select") as HTMLElement | null
        )?.focus();
      } else if (ARROWS[event.key] && selection.length > 0 && event.shiftKey) {
        event.preventDefault();
        commit(resizeBy(page, selection, ARROWS[event.key], event.altKey));
      } else if (ARROWS[event.key] && selection.length > 0) {
        event.preventDefault();
        commit(nudge(page, selection, ARROWS[event.key], event.altKey));
      } else if ((event.key === "Delete" || event.key === "Backspace") && selection.length > 0) {
        event.preventDefault();
        // The pipes first, while their places still name them; an object's pipes go with it.
        commit(deleteObjects(deletePipes(page, selection), selection));
        setSelection([]);
      } else if ((event.ctrlKey || event.metaKey) && ["c", "x", "v", "d"].includes(event.key.toLowerCase())) {
        // Copy, cut, paste and duplicate (§3.3 item 8).
        event.preventDefault();
        const actions: Record<string, EditAction> = { c: "copy", x: "cut", v: "paste", d: "duplicate" };
        actRef.current(actions[event.key.toLowerCase()]);
      } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "g" && selection.length > 0) {
        // Ctrl+G groups, Ctrl+Shift+G ungroups (§3.3 item 8).
        event.preventDefault();
        actRef.current(event.shiftKey ? "ungroup" : "group");
      } else if (
        (event.ctrlKey || event.metaKey) &&
        (event.key === "]" || event.key === "[" || event.key === "}" || event.key === "{") &&
        selection.length > 0
      ) {
        // The layer order: one step, or with Shift to the front or the back (§3.3 item 8).
        event.preventDefault();
        const forward = event.key === "]" || event.key === "}";
        actRef.current(event.shiftKey ? (forward ? "front" : "back") : forward ? "forward" : "backward");
      } else if (
        event.shiftKey &&
        !event.ctrlKey &&
        !event.metaKey &&
        ["Digit0", "Digit1", "Digit2"].includes(event.code)
      ) {
        // Shift+0 shows 100 %, Shift+1 fits the page, Shift+2 zooms to the selection (§3.3 item 8).
        event.preventDefault();
        if (event.code === "Digit0") setZoom(1);
        else if (event.code === "Digit1") fit();
        else zoomToSelection();
      } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
        // Ctrl+A selects every object the canvas lets the user pick (§3.3 item 8).
        event.preventDefault();
        setSelection(
          page.object
            .filter((object) => !object.locked && !hiddenObjects.includes(object.id))
            .map((object) => object.id),
        );
      } else if (event.key === "Escape") {
        setSelection([]);
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousedown", pressTakesKeys, true);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", pressTakesKeys, true);
    };
  }, [page, paused, selection, hiddenObjects, commit, setSelection, setZoom, fit, zoomToSelection, actRef]);
}
