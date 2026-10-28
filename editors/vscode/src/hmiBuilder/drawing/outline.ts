// A symbol's drawn outline (specification 39, §21.6): path data in the symbol's own pixels, as the
// symbol draws its walls, flattened into closed polygons, so that an instrument's line can end on
// the outline where it is drawn instead of on the box around it.

import type { Point } from "./pipes";

/** Straight pieces per Bézier curve. */
const CURVE_PIECES = 16;
/** The largest angle one straight piece of an arc turns through: under 0.1 px off a 50 px radius. */
const ARC_STEP = Math.PI / 32;
/** The numbers each command the outline reads takes. */
const NUMBERS: Partial<Record<string, number>> = { M: 2, L: 2, H: 1, V: 1, Q: 4, C: 6, A: 7 };

const flattened = new Map<string, Point[][]>();

/**
 * The closed polygons of path data written with absolute commands: M, L, H, V, Q, C, A and Z. Each
 * subpath closes back to its start; curves become short straight pieces. Data the outline cannot
 * read (a relative command, a missing number) ends it there.
 */
export function outlinePolygons(d: string): Point[][] {
  const known = flattened.get(d);
  if (known) return known;
  const polygons = flatten(d);
  flattened.set(d, polygons);
  return polygons;
}

function flatten(d: string): Point[][] {
  const tokens = d.match(/[A-Za-z]|[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?/g) ?? [];
  const polygons: Point[][] = [];
  let current: Point[] = [];
  let at: Point = [0, 0];
  let command = "";
  let index = 0;
  const close = (): void => {
    if (current.length > 2) polygons.push(current);
    if (current.length > 0) at = current[0];
    current = [];
  };
  const numbers = (count: number): number[] | undefined => {
    const taken = tokens.slice(index, index + count).map(Number);
    if (taken.length < count || taken.some((value) => !Number.isFinite(value))) return undefined;
    index += count;
    return taken;
  };
  while (index < tokens.length) {
    if (/^[A-Za-z]$/.test(tokens[index])) command = tokens[index++];
    if (command === "Z") {
      close();
      command = "";
      continue;
    }
    const count = NUMBERS[command];
    const values = count === undefined ? undefined : numbers(count);
    if (!values) break;
    switch (command) {
      case "M":
        close();
        at = [values[0], values[1]];
        current = [at];
        // Further pairs after a move are lines.
        command = "L";
        break;
      case "L":
        at = [values[0], values[1]];
        current.push(at);
        break;
      case "H":
        at = [values[0], at[1]];
        current.push(at);
        break;
      case "V":
        at = [at[0], values[0]];
        current.push(at);
        break;
      case "Q": {
        const end: Point = [values[2], values[3]];
        current.push(...curve(at, [values[0], values[1]], undefined, end));
        at = end;
        break;
      }
      case "C": {
        const end: Point = [values[4], values[5]];
        current.push(...curve(at, [values[0], values[1]], [values[2], values[3]], end));
        at = end;
        break;
      }
      case "A": {
        const end: Point = [values[5], values[6]];
        current.push(...arc(at, values[0], values[1], values[2], values[3] !== 0, values[4] !== 0, end));
        at = end;
        break;
      }
    }
  }
  close();
  return polygons;
}

/** A quadratic (one control point) or cubic Bézier curve from `from` to `to`, without `from`. */
function curve(from: Point, first: Point, second: Point | undefined, to: Point): Point[] {
  const points: Point[] = [];
  for (let piece = 1; piece <= CURVE_PIECES; piece += 1) {
    const t = piece / CURVE_PIECES;
    const u = 1 - t;
    const weights = second ? [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t] : [u * u, 2 * u * t, t * t];
    const controls = second ? [from, first, second, to] : [from, first, to];
    points.push([
      controls.reduce((sum, point, which) => sum + point[0] * weights[which], 0),
      controls.reduce((sum, point, which) => sum + point[1] * weights[which], 0),
    ]);
  }
  return points;
}

/** An elliptical arc as SVG draws it (SVG 1.1, appendix F.6.5), from `from` to `to`, without `from`. */
function arc(
  from: Point,
  rx: number,
  ry: number,
  rotationDegrees: number,
  large: boolean,
  sweep: boolean,
  to: Point,
): Point[] {
  if (rx === 0 || ry === 0 || (from[0] === to[0] && from[1] === to[1])) return [to];
  let [radiusX, radiusY] = [Math.abs(rx), Math.abs(ry)];
  const phi = (rotationDegrees * Math.PI) / 180;
  const [cos, sin] = [Math.cos(phi), Math.sin(phi)];
  const [dx, dy] = [(from[0] - to[0]) / 2, (from[1] - to[1]) / 2];
  const x1 = cos * dx + sin * dy;
  const y1 = -sin * dx + cos * dy;
  // Radii too small to reach the end grow until they just do.
  const reach = (x1 * x1) / (radiusX * radiusX) + (y1 * y1) / (radiusY * radiusY);
  if (reach > 1) {
    radiusX *= Math.sqrt(reach);
    radiusY *= Math.sqrt(reach);
  }
  const numerator = radiusX * radiusX * radiusY * radiusY - radiusX * radiusX * y1 * y1 - radiusY * radiusY * x1 * x1;
  const denominator = radiusX * radiusX * y1 * y1 + radiusY * radiusY * x1 * x1;
  const factor = (large !== sweep ? 1 : -1) * Math.sqrt(Math.max(0, numerator / denominator));
  const [cx1, cy1] = [(factor * radiusX * y1) / radiusY, (-factor * radiusY * x1) / radiusX];
  const centre: Point = [cos * cx1 - sin * cy1 + (from[0] + to[0]) / 2, sin * cx1 + cos * cy1 + (from[1] + to[1]) / 2];
  const angle = (ux: number, uy: number, vx: number, vy: number): number =>
    Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
  const start = angle(1, 0, (x1 - cx1) / radiusX, (y1 - cy1) / radiusY);
  let turn = angle((x1 - cx1) / radiusX, (y1 - cy1) / radiusY, (-x1 - cx1) / radiusX, (-y1 - cy1) / radiusY);
  if (!sweep && turn > 0) turn -= 2 * Math.PI;
  if (sweep && turn < 0) turn += 2 * Math.PI;
  const pieces = Math.max(4, Math.ceil(Math.abs(turn) / ARC_STEP));
  const points: Point[] = [];
  for (let piece = 1; piece < pieces; piece += 1) {
    const theta = start + (turn * piece) / pieces;
    const [ex, ey] = [radiusX * Math.cos(theta), radiusY * Math.sin(theta)];
    points.push([centre[0] + cos * ex - sin * ey, centre[1] + sin * ex + cos * ey]);
  }
  // The arc ends exactly where the path says.
  points.push(to);
  return points;
}
