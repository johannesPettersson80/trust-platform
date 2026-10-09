// The editor's panels (specification 39, §3.3 item 8): the arrangement the user left them in, as
// drawn in this window (a narrow editor shows a side panel from its strip for now), collapsing
// and opening the side panels, and dragging their edges; each change is remembered by the host.

import { useCallback, useEffect, useState } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";

import {
  Arrangement,
  DEFAULT_ARRANGEMENT,
  normalizeArrangement,
  resized,
  shownArrangement,
  type ShownArrangement,
} from "../arrangement";
import type { VsCodeApi } from "./vscodeApi";

export interface PanelArrangement {
  arrangement: Arrangement;
  /** The panels as this window draws them. */
  shown: ShownArrangement;
  /** The arrangement the user last left, told by the host. */
  adopt: (stored: unknown) => void;
  /** Changes the arrangement and asks the editor to remember it for the user. */
  remember: (next: Arrangement) => void;
  collapse: (side: "left" | "right") => void;
  open: (side: "left" | "right") => void;
  dragEdge: (edge: "left" | "right" | "bottom", event: ReactMouseEvent) => void;
}

export function usePanelArrangement(api: VsCodeApi): PanelArrangement {
  const [arrangement, setArrangement] = useState<Arrangement>(DEFAULT_ARRANGEMENT);
  const [peek, setPeek] = useState<"left" | "right">();
  const [windowSize, setWindowSize] = useState({ width: window.innerWidth, height: window.innerHeight });
  useEffect(() => {
    const onResize = (): void => setWindowSize({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  const adopt = useCallback((stored: unknown): void => setArrangement(normalizeArrangement(stored)), []);

  const shown = shownArrangement(arrangement, windowSize.width, windowSize.height, peek);
  /** Changes the arrangement and asks the editor to remember it for the user. */
  const remember = (next: Arrangement): void => {
    setArrangement(next);
    api.postMessage({ type: "arrangement", arrangement: next });
  };
  /** Collapses a side panel; one opened from its strip in a narrow editor goes back to its strip. */
  const collapse = (side: "left" | "right"): void => {
    if (shown.narrow) setPeek(undefined);
    else remember({ ...arrangement, [side]: { ...arrangement[side], collapsed: true } });
  };
  /** Opens a side panel from its strip: for now in a narrow editor, otherwise for good. */
  const open = (side: "left" | "right"): void => {
    if (shown.narrow) setPeek(side);
    else remember({ ...arrangement, [side]: { ...arrangement[side], collapsed: false } });
  };
  /** Drags a panel edge; the new size is remembered when the edge is let go. */
  const dragEdge = (edge: "left" | "right" | "bottom", event: ReactMouseEvent): void => {
    if (event.button !== 0) return;
    event.preventDefault();
    const start = edge === "bottom" ? event.clientY : event.clientX;
    const base = arrangement;
    let next = base;
    const move = (moved: MouseEvent): void => {
      next = resized(
        base,
        edge,
        (edge === "bottom" ? moved.clientY : moved.clientX) - start,
        shown.bottomHeight,
        windowSize.height,
      );
      setArrangement(next);
    };
    const up = (): void => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      if (next !== base) api.postMessage({ type: "arrangement", arrangement: next });
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };

  return { arrangement, shown, adopt, remember, collapse, open, dragEdge };
}
