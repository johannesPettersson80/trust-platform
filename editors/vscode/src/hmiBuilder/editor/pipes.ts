// Editing pipes (specification 39, §6): joining two ports, deleting pipes, bending a pipe by
// dragging one of its runs, splitting a pipe with a valve dropped onto it, and finding the port
// or the pipe under the pointer. A pipe is named `pipe:<n>` by its place among the pipes.

import type { HmiObjectModel, HmiPageModel, HmiPipeModel } from "../drawing/model";
import { CLEARANCE, pipeRoutes, Point, portPoint, STUB } from "../drawing/pipes";
import { afterDeleting, movedAttachments } from "./attachments";
import { addObject, drawnBox, snap } from "./editing";

/** How near the pointer must come to a port or a pipe to take it, in page pixels. */
const REACH = 8;

/** A pipe between two free ports; the page as it was for a port that has a pipe, or one port
 * twice. */
export function addPipe(page: HmiPageModel, from: string, to: string): HmiPageModel {
  if (from === to) return page;
  const taken = new Set(page.pipe.flatMap((pipe) => [pipe.from, pipe.to]));
  if (taken.has(from) || taken.has(to)) return page;
  return { ...page, pipe: [...page.pipe, { from, to }] };
}

/** Deletes pipes by name; the attachments to them go and those to later pipes follow (§6). */
export function deletePipes(page: HmiPageModel, ids: string[]): HmiPageModel {
  const gone = new Set(ids.map(pipeIndex).filter((index): index is number => index !== undefined));
  if (gone.size === 0) return page;
  return movedAttachments({ ...page, pipe: page.pipe.filter((_, index) => !gone.has(index)) }, afterDeleting(gone));
}

/** The place of a pipe named `pipe:<n>`. */
export function pipeIndex(id: string): number | undefined {
  const match = /^pipe:(\d+)$/.exec(id);
  return match ? Number(match[1]) : undefined;
}

/** The port nearest the pointer, within reach, written `object.port`; `ports` names each
 * component type's ports, as the catalogue does. */
export function portAt(page: HmiPageModel, point: Point, ports: Record<string, string[]>): string | undefined {
  let found: string | undefined;
  let nearest = REACH;
  for (const object of page.object) {
    for (const port of ports[object.type] ?? []) {
      const at = portPoint(object, port)?.at;
      if (!at) continue;
      const distance = Math.hypot(at[0] - point[0], at[1] - point[1]);
      if (distance <= nearest) {
        found = `${object.id}.${port}`;
        nearest = distance;
      }
    }
  }
  return found;
}

/** The topmost pipe whose route passes within reach of the pointer. */
export function pipeAt(page: HmiPageModel, point: Point, size: [number, number]): string | undefined {
  const routes = pipeRoutes(page, size);
  for (let index = routes.length - 1; index >= 0; index -= 1) {
    const route = routes[index];
    if (route && distanceTo(route, point) <= REACH) return `pipe:${index}`;
  }
  return undefined;
}

/** A pipe bent by dragging the run under `grabbed` to `to`: the run's two ends, moved onto the
 * grid line across it, become the pipe's waypoints, kept clear of the objects at the ports, so the
 * pipe turns into the moved run without going round them (§6, item 3). */
export function bendPipe(
  page: HmiPageModel,
  pipe: string,
  grabbed: Point,
  to: Point,
  size: [number, number],
): HmiPageModel {
  const index = pipeIndex(pipe);
  const route = index === undefined ? undefined : pipeRoutes(page, size)[index];
  if (index === undefined || !route) return page;
  const run = nearestRun(route, grabbed);
  if (run === undefined) return page;
  let [start, end] = [route[run], route[run + 1]];
  if (run === 0) start = clearEnd(page, page.pipe[index].from) ?? start;
  if (run === route.length - 2) end = clearEnd(page, page.pipe[index].to) ?? end;
  const across = start[1] === end[1] ? snap(to[1]) : snap(to[0]);
  const moved: Array<[number, number]> =
    start[1] === end[1]
      ? [
          [start[0], across],
          [end[0], across],
        ]
      : [
          [across, start[1]],
          [across, end[1]],
        ];
  // Keep all other authored turns. Replacing the entire waypoint list lost earlier bends.
  const points = [...route.slice(1, run), ...moved, ...route.slice(run + 2, -1)];
  const waypoints = points.filter(
    (point, at) => at === 0 || point[0] !== points[at - 1][0] || point[1] !== points[at - 1][1],
  );
  return { ...page, pipe: page.pipe.map((candidate, at) => (at === index ? { ...candidate, waypoints } : candidate)) };
}

/** A pipe drawn from a port and released on another pipe ends on a new junction there, centred
 * on that pipe, which it splits (§6, item 2); the branch enters the junction from its own side. */
export function branchPipe(
  page: HmiPageModel,
  from: string,
  point: Point,
  ports: Record<string, string[]>,
  size: [number, number],
): HmiPageModel {
  if (page.pipe.some((pipe) => pipe.from === from || pipe.to === from)) return page;
  const origin = endPoint(page, from);
  const index = pipeIndex(pipeAt(page, point, size) ?? "");
  const route = index === undefined ? undefined : pipeRoutes(page, size)[index];
  const run = route ? nearestRun(route, point) : undefined;
  if (!origin || !route || run === undefined) return page;
  const [a, b] = [route[run], route[run + 1]];
  const level = a[1] === b[1];
  const along = (low: number, high: number, at: number): number =>
    Math.max(Math.min(low, high), Math.min(Math.max(low, high), snap(at)));
  const centre: Point = level ? [along(a[0], b[0], point[0]), a[1]] : [a[0], along(a[1], b[1], point[1])];
  const half = JUNCTION / 2;
  const placed = addObject(
    page,
    { type: "junction", size: [JUNCTION, JUNCTION] },
    [centre[0] - half, centre[1] - half],
    true,
  );
  const split = splitPipe(placed.page, placed.id, ports, size);
  if (split === placed.page) return page;
  const side = level ? (origin[1] < centre[1] ? "top" : "bottom") : origin[0] < centre[0] ? "left" : "right";
  return addPipe(split, from, `${placed.id}.${side}`);
}

/** An object dropped onto a pipe it can sit in splits the pipe into two pipes through it (§6,
 * item 4): a valve through its inlet and outlet, a junction through its two sides along the pipe.
 * The object is moved onto the pipe's line, the part from the pipe's start keeps its flow
 * binding, both keep its style, and each bend stays on the part it lies on. */
export function splitPipe(
  page: HmiPageModel,
  objectId: string,
  ports: Record<string, string[]>,
  size: [number, number],
): HmiPageModel {
  const object = page.object.find((candidate) => candidate.id === objectId);
  if (!object || page.pipe.some((pipe) => [pipe.from, pipe.to].some((end) => endObject(end) === objectId))) return page;
  const names = ports[object.type] ?? [];
  const pairs = PASSAGES.filter((pair) => pair.every((name) => names.includes(name)));
  // Where the pipes run without the object in their way.
  const routes = pipeRoutes({ ...page, object: page.object.filter((candidate) => candidate.id !== objectId) }, size);
  for (const [index, route] of routes.entries()) {
    if (!route) continue;
    for (const pair of pairs) {
      const found = passage(object, pair, route);
      if (!found) continue;
      const { bind, waypoints = [], ...line } = page.pipe[index];
      const moved = {
        ...object,
        at: [object.at[0] + found.shift[0], object.at[1] + found.shift[1]] as [number, number],
      };
      // A bend lies on the route; those up to the split run stay with the first part.
      const place = (bend: [number, number]): number =>
        route.findIndex((point) => point[0] === bend[0] && point[1] === bend[1]);
      const before = waypoints.filter((bend) => place(bend) <= found.run);
      const after = waypoints.filter((bend) => place(bend) > found.run);
      const first: HmiPipeModel = {
        ...line,
        to: `${objectId}.${found.entry}`,
        ...(bind ? { bind } : {}),
        ...(before.length > 0 ? { waypoints: before } : {}),
      };
      const second: HmiPipeModel = {
        ...line,
        from: `${objectId}.${found.exit}`,
        ...(after.length > 0 ? { waypoints: after } : {}),
      };
      const split = {
        ...page,
        object: page.object.map((candidate) => (candidate.id === objectId ? moved : candidate)),
        pipe: [...page.pipe.slice(0, index), first, second, ...page.pipe.slice(index + 1)],
      };
      // An instrument on the split pipe stays on its first part; later pipes move on by one.
      return movedAttachments(split, (at) => (at <= index ? at : at + 1));
    }
  }
  return page;
}

/** A junction's size, as the catalogue gives it. */
const JUNCTION = 16;

/** The pairs of ports a pipe can pass through: a valve's, and a junction's two sides. */
const PASSAGES: Array<[string, string]> = [
  ["inlet", "outlet"],
  ["left", "right"],
  ["top", "bottom"],
];

/** How a pipe's route passes through a pair of an object's ports: the run that the pair lies on,
 * within reach, facing along it (its place in the route); the shift that puts the pair on the
 * run's line; and which port the pipe enters by and which it leaves by. */
function passage(
  object: HmiObjectModel,
  pair: [string, string],
  route: Point[],
): { run: number; shift: Point; entry: string; exit: string } | undefined {
  const [one, two] = pair.map((name) => portPoint(object, name));
  if (!one || !two) return undefined;
  const middle: Point = [(one.at[0] + two.at[0]) / 2, (one.at[1] + two.at[1]) / 2];
  const level = (direction: string): boolean => direction === "left" || direction === "right";
  for (let run = 0; run < route.length - 1; run += 1) {
    const [a, b] = [route[run], route[run + 1]];
    const runLevel = a[1] === b[1];
    if (level(one.direction) !== runLevel || level(two.direction) !== runLevel || toSegment(middle, a, b) > REACH)
      continue;
    // The pipe enters by the port that faces where it comes from.
    const forward = runLevel ? b[0] > a[0] : b[1] > a[1];
    const facing = runLevel ? (forward ? "left" : "right") : forward ? "up" : "down";
    const [entry, exit] = one.direction === facing ? pair : [pair[1], pair[0]];
    const shift: Point = runLevel ? [0, a[1] - middle[1]] : [a[0] - middle[0], 0];
    return { run, shift, entry, exit };
  }
  return undefined;
}

/** Where an end, `object.port`, lies on the page. */
function endPoint(page: HmiPageModel, end: string): Point | undefined {
  const object = page.object.find((candidate) => candidate.id === endObject(end));
  return object ? portPoint(object, endPort(end))?.at : undefined;
}

function endObject(end: string): string {
  return end.slice(0, end.lastIndexOf("."));
}

function endPort(end: string): string {
  return end.slice(end.lastIndexOf(".") + 1);
}

/** The point out of a port, in its direction, where a pipe is clear of the port's object: at
 * least a stub away, and a clearance beyond the object's box. */
function clearEnd(page: HmiPageModel, end: string): Point | undefined {
  const object = page.object.find((candidate) => candidate.id === endObject(end));
  const port = object ? portPoint(object, endPort(end)) : undefined;
  if (!object || !port) return undefined;
  const box = drawnBox(object);
  const [x, y] = port.at;
  switch (port.direction) {
    case "left":
      return [Math.min(x - STUB, box.x - CLEARANCE), y];
    case "right":
      return [Math.max(x + STUB, box.x + box.width + CLEARANCE), y];
    case "up":
      return [x, Math.min(y - STUB, box.y - CLEARANCE)];
    default:
      return [x, Math.max(y + STUB, box.y + box.height + CLEARANCE)];
  }
}

/** The run of a route nearest a point, within reach. */
function nearestRun(route: Point[], point: Point): number | undefined {
  let found: number | undefined;
  let nearest = REACH;
  for (let run = 0; run < route.length - 1; run += 1) {
    const distance = toSegment(point, route[run], route[run + 1]);
    if (distance <= nearest) {
      found = run;
      nearest = distance;
    }
  }
  return found;
}

function distanceTo(route: Point[], point: Point): number {
  let nearest = Infinity;
  for (let run = 0; run < route.length - 1; run += 1)
    nearest = Math.min(nearest, toSegment(point, route[run], route[run + 1]));
  return nearest;
}

function toSegment(point: Point, a: Point, b: Point): number {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const length = dx * dx + dy * dy;
  const t = length === 0 ? 0 : Math.max(0, Math.min(1, ((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length));
  return Math.hypot(point[0] - (a[0] + t * dx), point[1] - (a[1] + t * dy));
}
