// Helpers the runtime page and its popups share (specification 39, §4.9, §11.1): the value
// stream's address, its messages, a pointer's design position and the trend spans a drawing
// keeps samples for.

import { NOT_PENS } from "../drawing/components/trend";
import type { HmiPageModel } from "../drawing/model";
import type { StreamMessage } from "./liveValues";
import type { DisplayHost } from "./runtimeDisplay";

const DEFAULT_SPAN_S = 300;

/** The value stream for a page (`page=<name>`) or a popup (`popup=<name>`). */
export function socketUrl(location: DisplayHost["location"], query: string): string {
  const scheme = location.protocol === "https:" ? "wss:" : "ws:";
  return `${scheme}//${location.host}/ws/hmi?${query}`;
}

export function parseMessage(data: unknown): StreamMessage | undefined {
  if (typeof data !== "string") return undefined;
  try {
    const message: unknown = JSON.parse(data);
    return typeof message === "object" && message !== null ? (message as StreamMessage) : undefined;
  } catch {
    return undefined;
  }
}

/** A pointer's position in a drawing's design pixels: the drawing keeps its proportions and is
 * centred in its surface. */
export function designPoint(surface: HTMLElement, size: [number, number], event: MouseEvent): { x: number; y: number } {
  const svg = surface.querySelector("svg") ?? surface;
  const box = svg.getBoundingClientRect();
  const scale = Math.min(box.width / size[0], box.height / size[1]) || 1;
  const left = box.left + (box.width - size[0] * scale) / 2;
  const top = box.top + (box.height - size[1] * scale) / 2;
  return { x: (event.clientX - left) / scale, y: (event.clientY - top) / scale };
}

/** For each path a trend draws, the longest span it shows: the page's trends and those of the
 * panels placed on it, which draw as their popups do (§8.9). */
export function trendSpans(page: HmiPageModel, placed: HmiPageModel[] = []): Record<string, number> {
  const spans: Record<string, number> = {};
  const objects = [page, ...placed].flatMap((model) => model.object);
  for (const object of objects.filter((candidate) => candidate.type === "trend")) {
    const spanMs = (object.span ?? DEFAULT_SPAN_S) * 1000;
    for (const [name, binding] of Object.entries(object.bind)) {
      if (!NOT_PENS.includes(name)) spans[binding.path] = Math.max(spans[binding.path] ?? 0, spanMs);
    }
  }
  return spans;
}
