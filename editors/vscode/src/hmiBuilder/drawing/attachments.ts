// Instrument attachments (specification 39, §6 item 5): an instrument is joined to the equipment
// or the pipe it measures by a thin orthogonal line from the middle of its bubble: straight onto
// the symbol's side or the pipe when it stands over, under or beside it, else down or up to the
// symbol's middle and across into the middle of its facing side, where symbols meet their pipes.
// The line is drawn under the objects, so it leaves the bubble at its edge, and it follows
// whatever it joins when that moves. On a symbol that draws an outline, as a vessel its walls, the
// line ends on that outline, not on the box the symbol stands in (§21.6).

import type { DrawFrame, HmiObjectModel, HmiPageModel } from "./model";
import { outlinePolygons } from "./outline";
import { token } from "./parts";
import { pipeRoutes, Point } from "./pipes";
import { el, SvgNode } from "./svg";
import { symbolFor, symbolPlacement } from "./symbols";

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function drawAttachments(page: HmiPageModel, frame: DrawFrame): SvgNode[] {
  const attached = page.object.filter((object) => object.attach !== undefined);
  if (attached.length === 0) return [];
  const routes = attached.some((object) => object.attach?.startsWith("pipe:")) ? pipeRoutes(page, frame.size) : [];
  return attached.flatMap((object) => {
    const from = attachOrigin(object).centre;
    const path = connection(page, object.attach ?? "", routes, from);
    if (!path) return [];
    return [
      el("polyline", {
        "data-attachment": object.id,
        points: path.map((point) => point.join(",")).join(" "),
        fill: "none",
        stroke: token("muted"),
        "stroke-width": 1.5,
        "stroke-linejoin": "round",
        "stroke-linecap": "round",
      }),
    ];
  });
}

/** The line's corners from `from` to the target: its symbol, or the pipe's route. */
function connection(
  page: HmiPageModel,
  attach: string,
  routes: Array<Point[] | undefined>,
  from: Point,
): Point[] | undefined {
  const pipe = /^pipe:(\d+)$/.exec(attach);
  if (pipe) {
    const route = routes[Number(pipe[1])];
    const to = route ? nearestOnRoute(route, from) : undefined;
    if (!to) return undefined;
    return to[0] === from[0] || to[1] === from[1] ? [from, to] : [from, [from[0], to[1]], to];
  }
  const object = page.object.find((candidate) => candidate.id === attach);
  if (!object) return undefined;
  const outline = drawnOutline(object);
  return outline ? toOutline(from, outline) : toBox(from, symbolBox(object));
}

/** Onto a drawn outline: routed as onto the box around it, the last piece carried on until it meets
 * the outline. When the line passes between parts that stand apart, as a cascade's two tanks, it
 * goes to the nearest part instead. */
function toOutline(from: Point, outline: Point[][]): Point[] {
  const met = meet(toBox(from, boundsOf(outline.flat())), outline);
  if (met) return met;
  const away = (polygon: Point[]): number => distanceToBox(from, boundsOf(polygon));
  const nearest = outline.reduce((best, polygon) => (away(polygon) < away(best) ? polygon : best));
  const route = toBox(from, boundsOf(nearest));
  return meet(route, [nearest]) ?? route;
}

/** The route with its last piece carried on to where it first meets the outline, or undefined when
 * it misses it. A route of no length, from a bubble standing on the outline's box, stays. */
function meet(route: Point[], outline: Point[][]): Point[] | undefined {
  const [start, end] = route.slice(-2);
  if (start[0] === end[0] && start[1] === end[1]) return route;
  const direction: Point = [end[0] - start[0], end[1] - start[1]];
  let nearest: number | undefined;
  for (const polygon of outline) {
    for (let index = 0; index < polygon.length; index += 1) {
      const along = crossing(start, direction, polygon[index], polygon[(index + 1) % polygon.length]);
      if (along !== undefined && (nearest === undefined || along < nearest)) nearest = along;
    }
  }
  if (nearest === undefined) return undefined;
  return [...route.slice(0, -1), [round(start[0] + direction[0] * nearest), round(start[1] + direction[1] * nearest)]];
}

/** How far along `direction` from `start` the ray crosses the piece from `a` to `b`, in lengths of
 * `direction`; undefined when it does not, or runs along it. */
function crossing(start: Point, direction: Point, a: Point, b: Point): number | undefined {
  const piece: Point = [b[0] - a[0], b[1] - a[1]];
  const across = direction[0] * piece[1] - direction[1] * piece[0];
  if (Math.abs(across) < 1e-12) return undefined;
  const offset: Point = [a[0] - start[0], a[1] - start[1]];
  const along = (offset[0] * piece[1] - offset[1] * piece[0]) / across;
  const share = (offset[0] * direction[1] - offset[1] * direction[0]) / across;
  return along >= -1e-9 && share >= -1e-9 && share <= 1 + 1e-9 ? Math.max(0, along) : undefined;
}

/** The outline an object's symbol draws, placed on the page as the symbol is drawn; undefined when
 * its symbol draws none or the object is turned. */
function drawnOutline(object: HmiObjectModel): Point[][] | undefined {
  const definition = symbolFor(object.type);
  if (!definition?.outline || object.rotation % 360 !== 0) return undefined;
  const { left, top, scale } = symbolPlacement(object.size, definition.size, definition.label);
  // The symbol is drawn at its scale to two decimals.
  const factor = round(scale);
  const [x, y] = object.at;
  const polygons = outlinePolygons(definition.outline).map((polygon) =>
    polygon.map(([px, py]): Point => [x + left + px * factor, y + top + py * factor]),
  );
  return polygons.length > 0 ? polygons : undefined;
}

function boundsOf(points: Point[]): Box {
  const xs = points.map((point) => point[0]);
  const ys = points.map((point) => point[1]);
  const [x, y] = [Math.min(...xs), Math.min(...ys)];
  return { x, y, width: Math.max(...xs) - x, height: Math.max(...ys) - y };
}

function distanceToBox(point: Point, box: Box): number {
  const dx = Math.max(box.x - point[0], 0, point[0] - (box.x + box.width));
  const dy = Math.max(box.y - point[1], 0, point[1] - (box.y + box.height));
  return Math.hypot(dx, dy);
}

/** Onto a box: straight when `from` stands over, under or beside it, else down or up to its
 * middle and across into the middle of its facing side. */
function toBox(from: Point, box: Box): Point[] {
  const [x, y] = from;
  const [right, bottom] = [box.x + box.width, box.y + box.height];
  const overX = x >= box.x && x <= right;
  const overY = y >= box.y && y <= bottom;
  if (overX && overY) return [from, from];
  if (overX) return [from, [x, y < box.y ? box.y : bottom]];
  if (overY) return [from, [x < box.x ? box.x : right, y]];
  const middle = round(box.y + box.height / 2);
  return [from, [x, middle], [x < box.x ? box.x : right, middle]];
}

/** Where an object's symbol stands on the page; its whole box when it is no symbol or turned. */
export function symbolBox(object: HmiObjectModel): Box {
  const [x, y] = object.at;
  const [width, height] = object.size;
  const definition = symbolFor(object.type);
  if (!definition || object.rotation % 360 !== 0) return { x, y, width, height };
  const { left, top, scale } = symbolPlacement(object.size, definition.size, definition.label);
  // An instrument's line leaves its bubble: the first bubble of a pair.
  const bubble = definition.label === "beside" ? definition.size[1] : definition.size[0];
  return { x: x + left, y: y + top, width: round(bubble * scale), height: round(definition.size[1] * scale) };
}

/** An instrument's bubble on the page: where its line leaves and its handle stands (§6, item 5). */
export function attachOrigin(object: HmiObjectModel): { centre: Point; radius: number } {
  const box = symbolBox(object);
  return { centre: middle(box), radius: round(Math.min(box.width, box.height) / 2) };
}

function middle(box: Box): Point {
  return [round(box.x + box.width / 2), round(box.y + box.height / 2)];
}

function nearestOnRoute(route: Point[], point: Point): Point | undefined {
  let best: Point | undefined;
  let distance = Infinity;
  for (let index = 1; index < route.length; index += 1) {
    const [a, b] = [route[index - 1], route[index]];
    const candidate: Point = [
      clamp(point[0], Math.min(a[0], b[0]), Math.max(a[0], b[0])),
      clamp(point[1], Math.min(a[1], b[1]), Math.max(a[1], b[1])),
    ];
    const away = Math.hypot(candidate[0] - point[0], candidate[1] - point[1]);
    if (away < distance) {
      best = candidate;
      distance = away;
    }
  }
  return best;
}

function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}
