// What an operator does with a trend on the runtime display (specification 39, §5.9 item 1): a
// range chip shows that span, filled from the runtime's history; a box dragged on the plot zooms
// into its period, the wheel zooms the time axis at the pointer, and a drag along the time row
// pans, each pausing the live scroll; Back to live follows the live values again. Export saves
// the visible window as CSV; Expand shows the trend over the page.

import type { HmiPageModel, TrendView } from "../drawing/model";
import { loadHistory, LiveValueStore } from "./liveValues";

import { NOT_PENS } from "../drawing/components/trend";
/** The shortest and longest window. */
const SHORTEST_MS = 10_000;
const LONGEST_MS = 86_400_000;

type Point = { x: number; y: number };

/** A trend as drawn: its plot on the page and the window it shows. */
interface Drawn {
  plot: { x: number; y: number; width: number; height: number };
  start: number;
  end: number;
}

/** A box or a pan under way. */
type Gesture =
  { kind: "box"; objectId: string; drawn: Drawn } | { kind: "pan"; objectId: string; drawn: Drawn; fromX: number };

export class TrendActions {
  private gesture: Gesture | undefined;

  constructor(
    private readonly model: () => HmiPageModel,
    private readonly store: LiveValueStore,
    private readonly fetchJson: (url: string) => Promise<{ status: number; body: unknown }>,
    /** Where a trend's drawing is, to measure its plot and window. */
    private readonly surface: Element,
    private readonly now: () => number,
    /** Each trend's view, by object id; an expanded trend shares its page's. */
    readonly views: Record<string, TrendView> = {},
    /** Gives the browser a file to save. */
    private readonly download?: (name: string, text: string) => void,
    /** Shows a trend over the page. */
    private readonly expand?: (objectId: string) => void,
  ) {}

  /** A press on a trend: on the plot it starts a box, on the time row a pan; whether it was one. */
  press(objectId: string, at: Point, target: Element | null): boolean {
    const drawn = this.drawn(objectId);
    if (!drawn) return false;
    if (target?.closest('[data-part="trend-time"]')) {
      this.gesture = { kind: "pan", objectId, drawn, fromX: at.x };
      return true;
    }
    const { plot } = drawn;
    if (at.x < plot.x || at.x > plot.x + plot.width || at.y < plot.y || at.y > plot.y + plot.height) return false;
    const time = timeAt(drawn, at.x);
    this.gesture = { kind: "box", objectId, drawn };
    this.views[objectId] = { ...this.views[objectId], box: { fromMs: time, toMs: time } };
    return true;
  }

  /** The pointer moved; whether a gesture follows it. */
  drag(at: Point): boolean {
    const gesture = this.gesture;
    if (!gesture) return false;
    const view = this.views[gesture.objectId] ?? {};
    if (gesture.kind === "box" && view.box) {
      this.views[gesture.objectId] = { ...view, box: { ...view.box, toMs: timeAt(gesture.drawn, at.x) } };
    } else if (gesture.kind === "pan") {
      const { drawn } = gesture;
      const span = drawn.end - drawn.start;
      this.window(gesture.objectId, span, drawn.end - ((at.x - gesture.fromX) / drawn.plot.width) * span);
    }
    return true;
  }

  /** The pointer was released: a box wide enough zooms into its period. */
  release(at: Point): boolean {
    const gesture = this.gesture;
    this.gesture = undefined;
    if (!gesture) return false;
    if (gesture.kind === "box") {
      const { box, ...view } = this.views[gesture.objectId] ?? {};
      this.views[gesture.objectId] = view;
      const to = timeAt(gesture.drawn, at.x);
      const from = box?.fromMs ?? to;
      if (Math.abs(to - from) * 100 >= gesture.drawn.end - gesture.drawn.start)
        this.window(gesture.objectId, Math.abs(to - from), Math.max(from, to));
      // A tap, as on a touch screen, puts the cursor there.
      else this.views[gesture.objectId] = { ...this.views[gesture.objectId], cursorMs: to };
    }
    return true;
  }

  /** The pointer over a trend's plot puts the cursor there; off the plot the cursor goes. Whether
   * a cursor moved. */
  hover(objectId: string | undefined, at: Point): boolean {
    if (this.gesture) return false;
    let changed = false;
    for (const [id, view] of Object.entries(this.views)) {
      if (id !== objectId && view.cursorMs !== undefined) {
        const { cursorMs: _gone, ...rest } = view;
        this.views[id] = rest;
        changed = true;
      }
    }
    const drawn = objectId ? this.drawn(objectId) : undefined;
    if (!objectId || !drawn) return changed;
    const { plot } = drawn;
    const inside = at.x >= plot.x && at.x <= plot.x + plot.width && at.y >= plot.y && at.y <= plot.y + plot.height;
    const view = this.views[objectId] ?? {};
    if (inside) this.views[objectId] = { ...view, cursorMs: timeAt(drawn, at.x) };
    else if (view.cursorMs !== undefined) {
      const { cursorMs: _gone, ...rest } = view;
      this.views[objectId] = rest;
    } else return changed;
    return true;
  }

  /** The wheel over a trend's plot zooms its time axis at the pointer; whether it did. */
  wheel(objectId: string, at: Point, deltaY: number): boolean {
    const drawn = this.drawn(objectId);
    if (!drawn || deltaY === 0) return false;
    const { plot } = drawn;
    if (at.x < plot.x || at.x > plot.x + plot.width || at.y < plot.y || at.y > plot.y + plot.height) return false;
    const span = drawn.end - drawn.start;
    const time = timeAt(drawn, at.x);
    const share = (time - drawn.start) / span;
    const next = Math.min(LONGEST_MS, Math.max(SHORTEST_MS, Math.round(span * (deltaY < 0 ? 0.8 : 1.25))));
    this.window(objectId, next, time - share * next + next);
    return true;
  }

  /** The visible window as CSV: the time of each sample in it, ISO 8601 in UTC, and each shown
   * pen's value then, held from its last sample; empty before its first. */
  private export(objectId: string): void {
    const object = this.model().object.find((candidate) => candidate.id === objectId);
    const drawn = this.drawn(objectId);
    if (!object || !drawn || !this.download) return;
    const hidden = this.views[objectId]?.hidden ?? [];
    const history = this.store.frame(this.now()).history ?? {};
    const pens = Object.entries(object.bind)
      .filter(([name]) => !NOT_PENS.includes(name) && !hidden.includes(name))
      .sort(([first], [second]) => (first < second ? -1 : first > second ? 1 : 0))
      .map(([name, binding]) => ({ name, samples: history[binding.path] ?? [] }));
    const times = [...new Set(pens.flatMap((pen) => pen.samples.map(([time]) => time)))]
      .filter((time) => time >= drawn.start && time <= drawn.end)
      .sort((first, second) => first - second);
    const rows = times.map((time) => [
      new Date(time).toISOString(),
      ...pens.map((pen) => {
        const held = pen.samples.filter(([at]) => at <= time).pop();
        return held === undefined ? "" : String(held[1]);
      }),
    ]);
    const text = [["time", ...pens.map((pen) => pen.name)], ...rows].map((row) => row.join(",")).join("\n") + "\n";
    const stamp = new Date(this.now()).toISOString().replace(/[-:]/g, "").replace("T", "-").slice(0, 15);
    this.download(`${objectId}-${stamp}.csv`, text);
  }

  /** Shows `spanMs` ending at `endMs`: live when it reaches now, else paused there. */
  private window(objectId: string, spanMs: number, endMs: number): void {
    const { endMs: _paused, ...view } = this.views[objectId] ?? {};
    const end = Math.round(endMs);
    this.views[objectId] = end >= this.now() ? { ...view, spanMs } : { ...view, spanMs, endMs: end };
    if (spanMs > (view.spanMs ?? 0)) void this.fill(objectId, spanMs);
  }

  /** The trend's plot on the page and its window, as last drawn. */
  private drawn(objectId: string): Drawn | undefined {
    const object = this.model().object.find((candidate) => candidate.id === objectId && candidate.type === "trend");
    const group = this.surface.querySelector(`[data-object=${JSON.stringify(objectId)}]`);
    const trend = group?.matches(".hmi-trend") ? group : group?.querySelector(".hmi-trend");
    const plot = trend?.querySelector('[data-part="plot"]');
    if (!object || !trend || !plot) return undefined;
    const number = (element: Element, name: string): number => Number(element.getAttribute(name));
    return {
      plot: {
        x: object.at[0] + number(plot, "x"),
        y: object.at[1] + number(plot, "y"),
        width: number(plot, "width") || 1,
        height: number(plot, "height") || 1,
      },
      start: number(trend, "data-window-start"),
      end: number(trend, "data-window-end"),
    };
  }

  /** A click on one of a trend's chips; whether it was one. `redraw` shows the change. */
  click(clicked: Element | null, objectId: string | undefined, redraw: () => void): boolean {
    if (!clicked || !objectId) return false;
    const range = clicked.closest("[data-trend-range]")?.getAttribute("data-trend-range");
    if (range) {
      const spanMs = Number(range) * 1000;
      this.views[objectId] = { ...this.views[objectId], spanMs };
      void this.fill(objectId, spanMs).then(redraw);
      redraw();
      return true;
    }
    // A legend entry hides its pen, or shows it again.
    const pen = clicked.closest('[data-part="legend"][data-pen]')?.getAttribute("data-pen");
    if (pen) {
      const hidden = this.views[objectId]?.hidden ?? [];
      this.views[objectId] = {
        ...this.views[objectId],
        hidden: hidden.includes(pen) ? hidden.filter((name) => name !== pen) : [...hidden, pen],
      };
      redraw();
      return true;
    }
    if (clicked.closest('[data-trend-action="export"]')) {
      this.export(objectId);
      return true;
    }
    if (clicked.closest('[data-trend-action="expand"]')) {
      this.expand?.(objectId);
      return true;
    }
    if (clicked.closest('[data-trend-action="live"]')) {
      const { endMs: _paused, ...live } = this.views[objectId] ?? {};
      this.views[objectId] = live;
      redraw();
      return true;
    }
    return false;
  }

  /** The trend's pens keep `spanMs` of samples, filled from the runtime's history. */
  private async fill(objectId: string, spanMs: number): Promise<void> {
    const object = this.model().object.find((candidate) => candidate.id === objectId);
    if (!object) return;
    const spans: Record<string, number> = {};
    for (const [name, binding] of Object.entries(object.bind)) {
      if (NOT_PENS.includes(name)) continue;
      this.store.widen(binding.path, spanMs);
      spans[binding.path] = spanMs;
    }
    await loadHistory(this.fetchJson, spans, this.store);
  }
}

/** The time at a point across a trend's plot. */
function timeAt(drawn: Drawn, x: number): number {
  const share = Math.min(1, Math.max(0, (x - drawn.plot.x) / drawn.plot.width));
  return Math.round(drawn.start + share * (drawn.end - drawn.start));
}

/** The expanded trend over the page and its close button (§5.9 item 1). */
export const TREND_CSS = `
.hmi-trend-expanded { position: fixed; inset: 0; z-index: 25; padding: 16px; background: var(--hmi-page); }
.hmi-trend-expanded .hmi-trend-surface { width: 100%; height: 100%; }
.hmi-trend-expanded .hmi-trend-surface > svg { width: 100%; height: 100%; }
.hmi-trend-expanded .hmi-trend-close { position: absolute; top: 20px; right: 24px; z-index: 1; width: 40px; height: 40px; border-radius: 20px; border: 1px solid var(--hmi-border); background: var(--hmi-raised); color: var(--hmi-text); font: 600 18px var(--hmi-font); cursor: pointer; }
`;
