// The pipes of a page (specification 39, §6): drawn under the objects, a process line solid and a
// signal line dashed, along the computed route. A pipe shows flow only from its flow binding,
// while that value is a current observation: never from a pump's or a valve's state.

import type { DrawFrame, HmiObjectModel, HmiPageModel, HmiPipeModel, LiveValue } from "../model";
import { token } from "../parts";
import { el, SvgNode } from "../svg";
import { LABEL_HEIGHT, SYMBOLS, symbolPlacement, SymbolType } from "../symbols";
import { FLOW_PERIOD_MS, observed } from "../symbols/motion";
import { portPoint, PortPoint } from "./ports";
import { Box, Point, routePipe } from "./route";

/** Each pipe's route on the page, in the pipes' order; undefined for a pipe whose end is not on
 * the page, which is a problem of the page shown in the Problems view. */
export function pipeRoutes(page: HmiPageModel, size: [number, number]): Array<Point[] | undefined> {
  if (page.pipe.length === 0) return [];
  const obstacles = page.object.flatMap(solidBoxes);
  return page.pipe.map((pipe) => {
    const from = end(page, pipe.from);
    const to = end(page, pipe.to);
    return from && to ? routePipe(from, to, obstacles, pipe.waypoints ?? [], size) : undefined;
  });
}

export function drawPipes(page: HmiPageModel, frame: DrawFrame): SvgNode[] {
  const routes = pipeRoutes(page, frame.size);
  return page.pipe.flatMap((pipe, index) => {
    const points = routes[index];
    if (!points) return [];
    const d = `M${points.map(([x, y]) => `${round(x)} ${round(y)}`).join(" L")}`;
    const signal = pipe.style === "signal";
    const children: SvgNode[] = [
      el("path", {
        "data-part": "line",
        d,
        fill: "none",
        stroke: token("muted"),
        "stroke-width": signal ? 1.5 : 4,
        "stroke-linejoin": "round",
        "stroke-linecap": "round",
        "stroke-dasharray": signal ? "6 5" : undefined,
        opacity: signal ? 0.9 : 0.75,
      }),
      ...flow(pipe, d, frame),
    ];
    return [
      el("g", { class: "hmi-pipe", "data-pipe": String(index), "data-style": signal ? "signal" : "process" }, children),
    ];
  });
}

/** The flow along a pipe, from its binding: forward for a positive number or TRUE, back for a
 * negative number, nothing at zero or while the value is stale, unknown or merely commanded. */
function flow(pipe: HmiPipeModel, d: string, frame: DrawFrame): SvgNode[] {
  const binding = pipe.bind?.flow;
  const live: LiveValue | undefined = binding ? frame.values[binding.path] : undefined;
  if (!observed(live)) return [];
  const value = live.value;
  const direction =
    value === true || (typeof value === "number" && value > 0) ? 1 : typeof value === "number" && value < 0 ? -1 : 0;
  if (direction === 0) return [];
  const estimated = live.state === "estimated";
  return [
    el("path", {
      "data-part": "flow",
      "data-estimated": estimated ? "true" : undefined,
      class: direction > 0 ? "hmi-flow" : "hmi-flow-back",
      d,
      fill: "none",
      stroke: token("accent"),
      "stroke-width": 4,
      "stroke-linecap": "round",
      "stroke-dasharray": "1 15",
      opacity: estimated ? 0.5 : undefined,
      style: `animation-delay: -${Math.floor(frame.nowMs ?? 0) % FLOW_PERIOD_MS}ms`,
    }),
  ];
}

function end(page: HmiPageModel, written: string): PortPoint | undefined {
  const at = written.lastIndexOf(".");
  if (at <= 0) return undefined;
  const object = page.object.find((candidate) => candidate.id === written.slice(0, at));
  return object ? portPoint(object, written.slice(at + 1)) : undefined;
}

/** What a pipe keeps clear of: a symbol and its label, or a whole object; nothing of a junction. */
export function solidBoxes(object: HmiObjectModel): Box[] {
  const [x, y] = object.at;
  const [width, height] = object.size;
  if (object.type === "junction") return [];
  const definition = SYMBOLS[object.type as SymbolType];
  // A rotated symbol, or one with its label beside it, stands in its whole box.
  if (!definition || definition.label === "beside" || (object.rotation ?? 0) % 360 !== 0)
    return [{ x, y, width, height }];
  const { left, top, scale, area } = symbolPlacement(object.size, definition.size);
  return [
    { x: x + left, y: y + top, width: definition.size[0] * scale, height: definition.size[1] * scale },
    { x, y: y + area, width, height: LABEL_HEIGHT },
  ];
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}
