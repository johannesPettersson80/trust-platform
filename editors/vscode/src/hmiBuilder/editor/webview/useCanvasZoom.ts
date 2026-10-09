// The canvas's zoom and pan (specification 39, §3.3 item 8): fitting the page into the window, a
// zoom that keeps the design point under the pointer, the selection filling the window, Ctrl+wheel
// and a pinch at the pointer, Space-drag and middle-drag panning, the first view fitted, and the
// user's own zoom remembered for the page.

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";

import type { HmiPageModel } from "../../drawing/model";
import { drawnBox, type Point } from "../editing";
import { FIRST_VIEW_MIN_ZOOM, fittedZoom } from "../viewport";
import type { VsCodeApi } from "./vscodeApi";

export interface CanvasZoom {
  zoom: number;
  setZoom: (zoom: number) => void;
  /** Fits the page into the window. */
  fit: () => void;
  /** The selection fills the window, centred. */
  zoomToSelection: () => void;
  /** The zoom the page was last left at, told by the host: the page opens at it, not fitted. */
  restore: (zoom: number) => void;
}

/** The zoom of the canvas `canvasRef` draws a page of `size` design units in. */
export function useCanvasZoom(
  api: VsCodeApi,
  canvasRef: RefObject<HTMLDivElement | null>,
  page: HmiPageModel | undefined,
  selection: string[],
  size: [number, number],
): CanvasZoom {
  const [zoom, setZoom] = useState(1);
  /** Where a zoom at the pointer keeps its design point: the pointer, and the point under it. */
  const zoomAnchor = useRef<{ client: Point; design: Point }>(undefined);
  /** The zoom last told to the host, or told by it, so only the user's own changes are remembered. */
  const zoomKnown = useRef<number>(undefined);
  const fitted = useRef(false);

  const fit = useCallback((): void => {
    const area = canvasRef.current;
    if (!area || area.clientWidth === 0) return;
    setZoom(fittedZoom(size, [area.clientWidth, area.clientHeight]));
  }, [canvasRef, size]);

  /** A zoom that keeps the design point under `client` where it is. */
  const zoomAt = useCallback(
    (next: number, client: Point): void => {
      const sheet = canvasRef.current?.querySelector('[data-part="canvas-page"]')?.getBoundingClientRect();
      if (sheet)
        zoomAnchor.current = {
          client,
          design: [(client[0] - sheet.left) / zoom, (client[1] - sheet.top) / zoom],
        };
      setZoom(Math.max(0.1, Math.min(4, Math.round(next * 100) / 100)));
    },
    [canvasRef, zoom],
  );

  /** The selection fills the window, centred (§3.3 item 8). */
  const zoomToSelection = useCallback((): void => {
    const area = canvasRef.current;
    const chosen = page?.object.filter((object) => selection.includes(object.id)) ?? [];
    if (!area || area.clientWidth === 0 || chosen.length === 0) return;
    const boxes = chosen.map(drawnBox);
    const left = Math.min(...boxes.map((box) => box.x));
    const top = Math.min(...boxes.map((box) => box.y));
    const width = Math.max(...boxes.map((box) => box.x + box.width)) - left;
    const height = Math.max(...boxes.map((box) => box.y + box.height)) - top;
    const next = Math.max(
      0.1,
      Math.min(4, Math.round(Math.min((area.clientWidth - 64) / width, (area.clientHeight - 64) / height) * 100) / 100),
    );
    const rect = area.getBoundingClientRect();
    zoomAnchor.current = {
      client: [rect.left + rect.width / 2, rect.top + rect.height / 2],
      design: [left + width / 2, top + height / 2],
    };
    setZoom(next);
  }, [canvasRef, page, selection]);

  // After a zoom at a point, the scroll puts the design point back under it.
  useLayoutEffect(() => {
    const anchor = zoomAnchor.current;
    const area = canvasRef.current;
    zoomAnchor.current = undefined;
    const sheet = area?.querySelector('[data-part="canvas-page"]')?.getBoundingClientRect();
    if (!anchor || !area || !sheet) return;
    area.scrollLeft += sheet.left + anchor.design[0] * zoom - anchor.client[0];
    area.scrollTop += sheet.top + anchor.design[1] * zoom - anchor.client[1];
  }, [canvasRef, zoom]);

  // The user's own zoom is remembered for the page, a moment after the last change.
  useEffect(() => {
    if (zoomKnown.current === zoom) return;
    const timer = setTimeout(() => {
      zoomKnown.current = zoom;
      api.postMessage({ type: "zoom", zoom });
    }, 300);
    return () => clearTimeout(timer);
  }, [zoom, api]);

  // Ctrl+wheel and a pinch zoom at the pointer; Space-drag and middle-drag pan (§3.3 item 8). The
  // listeners go on the canvas as soon as it is drawn.
  const space = useRef(false);
  useLayoutEffect(() => {
    const area = canvasRef.current;
    if (!area) return;
    const onWheel = (event: WheelEvent): void => {
      if (!event.ctrlKey && !event.metaKey) return;
      event.preventDefault();
      zoomAt(zoom * Math.exp(-event.deltaY * 0.002), [event.clientX, event.clientY]);
    };
    const onPress = (event: MouseEvent): void => {
      if (!(space.current && event.button === 0) && event.button !== 1) return;
      event.preventDefault();
      event.stopPropagation();
      const start: Point = [event.clientX, event.clientY];
      const scroll: Point = [area.scrollLeft, area.scrollTop];
      area.classList.add("hb-panning");
      const move = (next: MouseEvent): void => {
        area.scrollLeft = scroll[0] + start[0] - next.clientX;
        area.scrollTop = scroll[1] + start[1] - next.clientY;
      };
      const up = (next: MouseEvent): void => {
        move(next);
        area.classList.remove("hb-panning");
        window.removeEventListener("mousemove", move);
        window.removeEventListener("mouseup", up);
      };
      window.addEventListener("mousemove", move);
      window.addEventListener("mouseup", up);
    };
    const onKey = (event: KeyboardEvent): void => {
      if (event.code !== "Space") return;
      const target = event.target as HTMLElement | null;
      if (target && ["INPUT", "SELECT", "TEXTAREA", "BUTTON"].includes(target.tagName)) return;
      space.current = event.type === "keydown";
      area.classList.toggle("hb-pan-ready", space.current);
      if (event.type === "keydown") event.preventDefault();
    };
    area.addEventListener("wheel", onWheel, { passive: false });
    area.addEventListener("mousedown", onPress, { capture: true });
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKey);
    return () => {
      area.removeEventListener("wheel", onWheel);
      area.removeEventListener("mousedown", onPress, { capture: true });
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", onKey);
    };
  }, [canvasRef, zoom, zoomAt, page]);

  useEffect(() => {
    const area = canvasRef.current;
    if (page && !fitted.current && area?.clientWidth) {
      fitted.current = true;
      setZoom(Math.max(FIRST_VIEW_MIN_ZOOM, fittedZoom(size, [area.clientWidth, area.clientHeight])));
    }
  }, [canvasRef, page, size]);

  const restore = useCallback((next: number): void => {
    zoomKnown.current = next;
    fitted.current = true;
    setZoom(next);
  }, []);

  return { zoom, setZoom, fit, zoomToSelection, restore };
}
