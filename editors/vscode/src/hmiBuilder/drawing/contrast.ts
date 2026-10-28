// Text contrast in the drawing (specification 39, §14.1 items 1 and 4): what each text lies on,
// as only the drawing knows it. How well it stands out with a palette, and what it needs, are the
// language server's rules (`trust-lsp.hmiQuality`), which take these texts.

import type { SvgNode } from "./svg";

/** One colour behind a text: a token with the opacity it is drawn at, or a mix of two tokens. */
export interface Backing {
  token: string;
  opacity: number;
  /** Part of the way toward a second token, as a gradient between two stops. */
  mix?: { token: string; share: number };
}

/** A text of the drawing with what lies behind it. */
export interface DrawnText {
  text: string;
  /** The text's colour token. */
  colour: string;
  /** The shapes under the text's middle, topmost first; a gradient gives each of its stops. */
  behind: Backing[][];
  size: number;
  bold: boolean;
  /** A text of symbols only, such as "?" or "!": a graphic, not words or numbers. */
  sign: boolean;
  /** The object the text belongs to. */
  object?: string;
}

/** A shape's paint: one backing, or a gradient's stops. */
type Paint = Backing[];

/** A gradient's stops, and for a linear one its direction within the shape's box. */
interface Gradient {
  linear?: { x1: number; y1: number; x2: number; y2: number };
  stops: Array<Backing & { offset: number }>;
}

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface Shape {
  contains(x: number, y: number): boolean;
  paint: Paint;
  /** A linear gradient, sampled where a text lies on it. */
  gradient?: { linear: NonNullable<Gradient["linear"]>; stops: Gradient["stops"]; box: Box; opacity: number };
}

interface Style {
  fill?: string;
  size?: string;
  weight?: string;
  anchor?: string;
  opacity: number;
}

/** The token of `var(--hmi-name)`. */
function tokenOf(value: string | undefined): string | undefined {
  return /^var\(--hmi-([a-z0-9-]+)\)$/.exec(value?.trim() ?? "")?.[1];
}

function number(value: string | undefined, fallback = 0): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

/** The translation of a `transform`; a rotation is left out. */
function translation(transform: string | undefined): [number, number] {
  const match = /translate\(\s*(-?[\d.]+)[\s,]+(-?[\d.]+)\s*\)/.exec(transform ?? "");
  return match ? [Number(match[1]), Number(match[2])] : [0, 0];
}

/** The corner points of a path made of straight lines, or of the ends of its curves. */
function pathPoints(d: string): Array<[number, number]> {
  const points: Array<[number, number]> = [];
  const commands = d.match(/[a-zA-Z][^a-zA-Z]*/g) ?? [];
  let [x, y] = [0, 0];
  for (const command of commands) {
    const letter = command[0];
    const numbers = (command.slice(1).match(/-?\d*\.?\d+(?:e-?\d+)?/gi) ?? []).map(Number);
    if (letter === "H") x = numbers[numbers.length - 1] ?? x;
    else if (letter === "V") y = numbers[numbers.length - 1] ?? y;
    else if (letter === "Z" || letter === "z") continue;
    else if (numbers.length >= 2) [x, y] = [numbers[numbers.length - 2], numbers[numbers.length - 1]];
    else continue;
    points.push([x, y]);
  }
  return points;
}

function inPolygon(points: Array<[number, number]>, x: number, y: number): boolean {
  let inside = false;
  for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
    const [xi, yi] = points[i];
    const [xj, yj] = points[j];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

/** A filled shape at its place on the page, or none when it paints nothing a text could lie on. */
function shapeOf(
  node: SvgNode,
  dx: number,
  dy: number,
  style: Style,
  gradients: Map<string, Gradient>,
): Shape | undefined {
  const fill = node.attrs.fill ?? style.fill;
  const opacity = style.opacity * number(node.attrs["fill-opacity"], 1);
  const token = tokenOf(fill);
  const gradient = gradients.get(/^url\(#([^)]+)\)$/.exec(fill ?? "")?.[1] ?? "");
  const paint: Paint | undefined = token
    ? [{ token, opacity }]
    : gradient
      ? gradient.stops.map((stop) => ({ token: stop.token, opacity: stop.opacity * opacity }))
      : undefined;
  if (!paint || opacity <= 0) return undefined;
  const a = node.attrs;
  let area: { box: Box; contains: Shape["contains"] } | undefined;
  switch (node.tag) {
    case "rect": {
      const box = { x: dx + number(a.x), y: dy + number(a.y), width: number(a.width), height: number(a.height) };
      area = {
        box,
        contains: (px, py) => px >= box.x && px <= box.x + box.width && py >= box.y && py <= box.y + box.height,
      };
      break;
    }
    case "circle": {
      const [cx, cy, r] = [dx + number(a.cx), dy + number(a.cy), number(a.r)];
      area = {
        box: { x: cx - r, y: cy - r, width: 2 * r, height: 2 * r },
        contains: (px, py) => (px - cx) ** 2 + (py - cy) ** 2 <= r * r,
      };
      break;
    }
    case "ellipse": {
      const [cx, cy, rx, ry] = [dx + number(a.cx), dy + number(a.cy), number(a.rx), number(a.ry)];
      area = {
        box: { x: cx - rx, y: cy - ry, width: 2 * rx, height: 2 * ry },
        contains: (px, py) => rx > 0 && ry > 0 && ((px - cx) / rx) ** 2 + ((py - cy) / ry) ** 2 <= 1,
      };
      break;
    }
    case "path":
    case "polygon": {
      const raw =
        node.tag === "path"
          ? pathPoints(a.d ?? "")
          : pathPoints(`M${a.points ?? ""}`.replace(/(-?[\d.]+)[\s,]+(-?[\d.]+)/g, "L$1 $2").replace(/^ML/, "M"));
      const points = raw.map(([x, y]) => [dx + x, dy + y] as [number, number]);
      if (points.length < 3) return undefined;
      const xs = points.map(([x]) => x);
      const ys = points.map(([, y]) => y);
      const box = {
        x: Math.min(...xs),
        y: Math.min(...ys),
        width: Math.max(...xs) - Math.min(...xs),
        height: Math.max(...ys) - Math.min(...ys),
      };
      area = { box, contains: (px, py) => inPolygon(points, px, py) };
      break;
    }
    default:
      return undefined;
  }
  return {
    paint,
    contains: area.contains,
    ...(gradient?.linear
      ? { gradient: { linear: gradient.linear, stops: gradient.stops, box: area.box, opacity } }
      : {}),
  };
}

/** The colour of a linear gradient at a point, as a mix of its two nearest stops. */
function sampleGradient(gradient: NonNullable<Shape["gradient"]>, x: number, y: number): Backing {
  const { linear, stops, box, opacity } = gradient;
  const [px, py] = [box.width > 0 ? (x - box.x) / box.width : 0, box.height > 0 ? (y - box.y) / box.height : 0];
  const [vx, vy] = [linear.x2 - linear.x1, linear.y2 - linear.y1];
  const length = vx * vx + vy * vy;
  const t = Math.min(1, Math.max(0, length > 0 ? ((px - linear.x1) * vx + (py - linear.y1) * vy) / length : 0));
  const after = stops.findIndex((stop) => stop.offset >= t);
  if (after <= 0)
    return { token: stops[Math.max(0, after)].token, opacity: stops[Math.max(0, after)].opacity * opacity };
  const [from, to] = [stops[after - 1], stops[after]];
  const share = to.offset > from.offset ? (t - from.offset) / (to.offset - from.offset) : 1;
  return {
    token: from.token,
    opacity: (from.opacity + (to.opacity - from.opacity) * share) * opacity,
    ...(share > 0 ? { mix: { token: to.token, share } } : {}),
  };
}

/** The drawing's gradients by id. */
function gradientStops(svg: SvgNode): Map<string, Gradient> {
  const gradients = new Map<string, Gradient>();
  const visit = (node: SvgNode): void => {
    if ((node.tag === "linearGradient" || node.tag === "radialGradient") && node.attrs.id) {
      const stops = node.children
        .filter((child) => child.tag === "stop")
        .map((stop) => ({
          token: tokenOf(stop.attrs["stop-color"]) ?? "",
          opacity: number(stop.attrs["stop-opacity"], 1),
          offset: number(stop.attrs.offset),
        }))
        .filter((stop) => stop.token !== "");
      const a = node.attrs;
      // A linear gradient runs along its box, left to right unless it says otherwise.
      const linear =
        node.tag === "linearGradient"
          ? { x1: number(a.x1, 0), y1: number(a.y1, 0), x2: number(a.x2, 1), y2: number(a.y2, 0) }
          : undefined;
      if (stops.length > 0) gradients.set(node.attrs.id, { stops, ...(linear ? { linear } : {}) });
    }
    node.children.forEach(visit);
  };
  visit(svg);
  return gradients;
}

/** Every text of a drawing with what lies under its middle, topmost first. */
export function drawnTexts(svg: SvgNode): DrawnText[] {
  const gradients = gradientStops(svg);
  const shapes: Shape[] = [];
  const texts: DrawnText[] = [];
  const walk = (node: SvgNode, dx: number, dy: number, style: Style, object: string | undefined): void => {
    if (["defs", "title", "clipPath", "mask", "pattern"].includes(node.tag)) return;
    const [tx, ty] = translation(node.attrs.transform);
    const [x0, y0] = [dx + tx, dy + ty];
    const owner = node.attrs["data-object"] ?? object;
    const own: Style = {
      fill: node.attrs.fill ?? style.fill,
      size: node.attrs["font-size"] ?? style.size,
      weight: node.attrs["font-weight"] ?? style.weight,
      anchor: node.attrs["text-anchor"] ?? style.anchor,
      opacity: style.opacity * number(node.attrs.opacity, 1),
    };
    if (node.tag === "text" || node.tag === "tspan") {
      const content = node.text ?? "";
      const colour = tokenOf(own.fill);
      if (content.trim() !== "" && colour) {
        const size = number(own.size, 16);
        const width = 0.55 * size * content.length;
        const x = x0 + number(node.attrs.x);
        const left = own.anchor === "middle" ? x - width / 2 : own.anchor === "end" ? x - width : x;
        const [mx, my] = [left + width / 2, y0 + number(node.attrs.y) - 0.35 * size];
        // A linear gradient is taken at the text's two ends and its middle.
        const behind = shapes
          .filter((shape) => shape.contains(mx, my))
          .reverse()
          .map((shape) =>
            shape.gradient ? [left, mx, left + width].map((x) => sampleGradient(shape.gradient!, x, my)) : shape.paint,
          );
        const weight = own.weight === "bold" ? 700 : number(own.weight, 400);
        const sign = !/[\p{L}\p{N}]/u.test(content);
        texts.push({
          text: content,
          colour,
          behind,
          size,
          bold: weight >= 700,
          sign,
          ...(owner ? { object: owner } : {}),
        });
      }
      node.children.forEach((child) => walk(child, x0, y0, own, owner));
      return;
    }
    const shape = shapeOf(node, x0, y0, own, gradients);
    if (shape) shapes.push(shape);
    node.children.forEach((child) =>
      walk(child, x0, y0, { ...own, fill: node.tag === "g" ? own.fill : style.fill }, owner),
    );
  };
  walk(svg, 0, 0, { opacity: 1 }, undefined);
  return texts;
}
