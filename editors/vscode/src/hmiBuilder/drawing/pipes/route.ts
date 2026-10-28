// A pipe's route (specification 39, §6, item 3): orthogonal, around the objects, with few bends,
// leaving each port the way it faces and through the bends the user placed. The route is found
// on the grid of the lines through the ports and the objects' edges, by A* with a cost for every
// bend, so the same page always gives the same route.

import type { PortDirection, PortPoint } from "./ports";

export type Point = [number, number];

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** How far a pipe runs straight out of its port before it may turn. */
export const STUB = 16;
/** The room a pipe keeps from an object. */
export const CLEARANCE = 8;
/** What a bend costs, in pixels of length. */
const BEND = 40;
/** Routes kept for redrawing an unchanged layout. */
const CACHE_SIZE = 256;

const STEPS: Record<PortDirection, Point> = { left: [-1, 0], right: [1, 0], up: [0, -1], down: [0, 1] };
const OPPOSITE: Record<PortDirection, PortDirection> = { left: "right", right: "left", up: "down", down: "up" };
const DIRECTIONS: PortDirection[] = ["left", "right", "up", "down"];

const cache = new Map<string, Point[]>();

/** The route from one port to another, through the user's bends, as the points of its line. */
export function routePipe(
  start: PortPoint,
  end: PortPoint,
  obstacles: Box[],
  waypoints: Point[],
  bounds: [number, number],
): Point[] {
  const key = JSON.stringify([start, end, obstacles, waypoints, bounds]);
  const known = cache.get(key);
  if (known) return known;
  const route = simplified(findRoute(start, end, obstacles, waypoints, bounds));
  if (cache.size >= CACHE_SIZE) cache.delete(cache.keys().next().value as string);
  cache.set(key, route);
  return route;
}

function findRoute(
  start: PortPoint,
  end: PortPoint,
  obstacles: Box[],
  waypoints: Point[],
  bounds: [number, number],
): Point[] {
  const boxes = obstacles.map((box) => ({
    x: box.x - CLEARANCE,
    y: box.y - CLEARANCE,
    width: box.width + 2 * CLEARANCE,
    height: box.height + 2 * CLEARANCE,
  }));
  // Two ports facing each other on one clear line are joined straight.
  if (waypoints.length === 0 && facing(start, end) && !boxes.some((box) => crosses([start.at, end.at], box)))
    return [start.at, end.at];
  const exit = moved(start.at, start.direction, STUB);
  const entry = moved(end.at, end.direction, STUB);
  // Only containers that still enclose the outward stub are omitted. A symbol's body
  // remains an obstacle: its port may touch the body without permitting a route through it.
  const solid = boxes.filter((box) => ![exit, entry].some((point) => inside(point, box)));
  const stops = [exit, ...waypoints, entry];
  const grid = new Grid(stops, solid, bounds);
  const path: Point[] = [start.at];
  let heading = start.direction;
  for (let leg = 0; leg < stops.length - 1; leg += 1) {
    const last = leg === stops.length - 2;
    const found = grid.search(stops[leg], stops[leg + 1], heading, last ? OPPOSITE[end.direction] : undefined);
    if (!found) return [start.at, exit, [entry[0], exit[1]], entry, end.at];
    path.push(...found.points);
    heading = found.heading;
  }
  path.push(end.at);
  return path;
}

function facing(start: PortPoint, end: PortPoint): boolean {
  const [dx, dy] = [end.at[0] - start.at[0], end.at[1] - start.at[1]];
  if (start.direction !== OPPOSITE[end.direction]) return false;
  const [sx, sy] = STEPS[start.direction];
  return (sx !== 0 && dy === 0 && Math.sign(dx) === sx) || (sy !== 0 && dx === 0 && Math.sign(dy) === sy);
}

function moved(point: Point, direction: PortDirection, distance: number): Point {
  const [sx, sy] = STEPS[direction];
  return [point[0] + sx * distance, point[1] + sy * distance];
}

function inside(point: Point, box: Box): boolean {
  return point[0] > box.x && point[0] < box.x + box.width && point[1] > box.y && point[1] < box.y + box.height;
}

function crosses([a, b]: [Point, Point], box: Box): boolean {
  const [x1, x2] = [Math.min(a[0], b[0]), Math.max(a[0], b[0])];
  const [y1, y2] = [Math.min(a[1], b[1]), Math.max(a[1], b[1])];
  return x1 < box.x + box.width && x2 > box.x && y1 < box.y + box.height && y2 > box.y;
}

/** The points without repeats and without the middle of a straight run. */
function simplified(points: Point[]): Point[] {
  const kept: Point[] = [];
  for (const point of points) {
    const last = kept[kept.length - 1];
    if (last && last[0] === point[0] && last[1] === point[1]) continue;
    const before = kept[kept.length - 2];
    if (
      last &&
      before &&
      ((before[0] === last[0] && last[0] === point[0]) || (before[1] === last[1] && last[1] === point[1]))
    )
      kept.pop();
    kept.push(point);
  }
  return kept;
}

/** The grid of the lines through the stops and the objects' edges, within the page. */
class Grid {
  private readonly xs: number[];
  private readonly ys: number[];

  constructor(
    stops: Point[],
    private readonly boxes: Box[],
    bounds: [number, number],
  ) {
    const within = (value: number, limit: number): boolean => value >= 0 && value <= limit;
    this.xs = unique([
      ...stops.map((point) => point[0]),
      ...boxes.flatMap((box) => [box.x, box.x + box.width]).filter((x) => within(x, bounds[0])),
    ]);
    this.ys = unique([
      ...stops.map((point) => point[1]),
      ...boxes.flatMap((box) => [box.y, box.y + box.height]).filter((y) => within(y, bounds[1])),
    ]);
  }

  /** The cheapest way from one stop to the next, starting with a heading, with a bend's cost
   * when it does not arrive with the heading asked for. */
  search(
    from: Point,
    to: Point,
    heading: PortDirection,
    arriving?: PortDirection,
  ): { points: Point[]; heading: PortDirection } | undefined {
    const [width, height] = [this.xs.length, this.ys.length];
    const node = (point: Point): number =>
      this.xs.indexOf(rounded(point[0])) + this.ys.indexOf(rounded(point[1])) * width;
    const start = node(from);
    const goal = node(to);
    const goalPoint = to;
    const state = (cell: number, direction: number): number => cell * 4 + direction;
    const best = new Map<number, number>();
    const parent = new Map<number, number>();
    const open = new Heap();
    const first = state(start, DIRECTIONS.indexOf(heading));
    best.set(first, 0);
    open.push(first, distance(from, goalPoint));
    while (open.size > 0) {
      const current = open.pop();
      const cell = Math.floor(current / 4);
      const direction = current % 4;
      const cost = best.get(current) ?? Infinity;
      if (cell === goal) {
        const points: Point[] = [];
        for (let at: number | undefined = current; at !== undefined; at = parent.get(at))
          points.unshift(this.point(Math.floor(at / 4)));
        return { points, heading: DIRECTIONS[direction] };
      }
      const [i, j] = [cell % width, Math.floor(cell / width)];
      DIRECTIONS.forEach((next, nextIndex) => {
        const [sx, sy] = STEPS[next];
        const [ni, nj] = [i + sx, j + sy];
        if (ni < 0 || nj < 0 || ni >= width || nj >= height) return;
        const target = this.point(ni + nj * width);
        const here = this.point(cell);
        if (!this.open([here, target])) return;
        const bends =
          (next === DIRECTIONS[direction] ? 0 : BEND) +
          (ni + nj * width === goal && arriving !== undefined && next !== arriving ? BEND : 0);
        // A pipe never doubles back on itself.
        if (next === OPPOSITE[DIRECTIONS[direction]]) return;
        const nextState = state(ni + nj * width, nextIndex);
        const nextCost = cost + distance(here, target) + bends;
        if (nextCost < (best.get(nextState) ?? Infinity)) {
          best.set(nextState, nextCost);
          parent.set(nextState, current);
          open.push(nextState, nextCost + distance(target, goalPoint));
        }
      });
    }
    return undefined;
  }

  private point(cell: number): Point {
    const width = this.xs.length;
    return [this.xs[cell % width], this.ys[Math.floor(cell / width)]];
  }

  /** Whether a step between two neighbouring grid points stays out of every object. */
  private open([a, b]: [Point, Point]): boolean {
    const middle: Point = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
    return !this.boxes.some((box) => inside(middle, box) || inside(b, box));
  }
}

function rounded(value: number): number {
  return Math.round(value * 100) / 100;
}

function unique(values: number[]): number[] {
  return [...new Set(values.map(rounded))].sort((a, b) => a - b);
}

function distance(a: Point, b: Point): number {
  return Math.abs(a[0] - b[0]) + Math.abs(a[1] - b[1]);
}

/** A binary heap of states by priority; equal priorities leave in the order they came. */
class Heap {
  private readonly items: Array<{ state: number; priority: number; order: number }> = [];
  private count = 0;

  get size(): number {
    return this.items.length;
  }

  push(state: number, priority: number): void {
    const items = this.items;
    items.push({ state, priority, order: this.count++ });
    let at = items.length - 1;
    while (at > 0) {
      const up = (at - 1) >> 1;
      if (!this.before(items[at], items[up])) break;
      [items[at], items[up]] = [items[up], items[at]];
      at = up;
    }
  }

  pop(): number {
    const items = this.items;
    const top = items[0];
    const last = items.pop()!;
    if (items.length > 0) {
      items[0] = last;
      let at = 0;
      for (;;) {
        const [left, right] = [at * 2 + 1, at * 2 + 2];
        let smallest = at;
        if (left < items.length && this.before(items[left], items[smallest])) smallest = left;
        if (right < items.length && this.before(items[right], items[smallest])) smallest = right;
        if (smallest === at) break;
        [items[at], items[smallest]] = [items[smallest], items[at]];
        at = smallest;
      }
    }
    return top.state;
  }

  private before(a: { priority: number; order: number }, b: { priority: number; order: number }): boolean {
    return a.priority < b.priority || (a.priority === b.priority && a.order < b.order);
  }
}
