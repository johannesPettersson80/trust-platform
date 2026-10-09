// A loop controller's signal lines (specification 39, §6 item 5): dashed, as signal pipes are, one
// from the instrument whose measurement it reads (`measures`) to its bubble, one from its bubble to
// the equipment it acts on (`acts_on`). Each is routed as a pipe is, orthogonal and around the
// other objects, leaving a bubble on the side that faces the other end and meeting equipment in
// the middle of its facing side.

import { attachOrigin, symbolBox } from "./attachments";
import type { DrawFrame, HmiObjectModel, HmiPageModel } from "./model";
import { token } from "./parts";
import { solidBoxes } from "./pipes/draw";
import type { PortDirection, PortPoint } from "./pipes/ports";
import { Box, Point, routePipe } from "./pipes/route";
import { el, SvgNode } from "./svg";

/** The objects that never stand in a signal line's way: the areas behind the process. */
const BACKGROUNDS = new Set(["panel", "group"]);

export function drawSignals(page: HmiPageModel, frame: DrawFrame): SvgNode[] {
  const byId = new Map(page.object.map((object) => [object.id, object]));
  return page.object.flatMap((controller) => {
    const lines: SvgNode[] = [];
    const measured = controller.measures ? byId.get(controller.measures) : undefined;
    const driven = controller.acts_on ? byId.get(controller.acts_on) : undefined;
    // The output leaves the side facing its equipment, the measurement enters another, so the two
    // never share an end; neither uses the right side, where the controller's text stands.
    const outputSide = driven ? controllerSide(controller, driven) : undefined;
    if (measured) {
      const side = controllerSide(controller, measured, outputSide);
      const route = signalRoute(
        page,
        frame.size,
        bubbleEnd(measured, controller),
        bubbleEnd(controller, measured, side),
        [measured, controller],
      );
      if (route) lines.push(signalLine(`${controller.id}:measures`, route));
    }
    if (driven) {
      const route = signalRoute(
        page,
        frame.size,
        bubbleEnd(controller, driven, outputSide),
        sideEnd(driven, controller),
        [controller, driven],
      );
      if (route) lines.push(signalLine(`${controller.id}:acts_on`, route));
    }
    return lines;
  });
}

function signalLine(id: string, route: Point[]): SvgNode {
  return el("polyline", {
    "data-signal": id,
    "data-style": "signal",
    points: route.map(([x, y]) => `${round(x)},${round(y)}`).join(" "),
    fill: "none",
    stroke: token("muted"),
    "stroke-width": 1.5,
    "stroke-dasharray": "6 5",
    "stroke-linejoin": "round",
  });
}

/** The route between two ends around every other object, or undefined when there is none. */
function signalRoute(
  page: HmiPageModel,
  size: [number, number],
  start: PortPoint,
  end: PortPoint,
  ends: HmiObjectModel[],
): Point[] | undefined {
  const obstacles = page.object
    .filter((object) => !ends.includes(object) && !BACKGROUNDS.has(object.type))
    .flatMap(solidBoxes);
  const route = routePipe(start, end, obstacles, [], size);
  return route.length >= 2 ? route : undefined;
}

/** The side of a controller's bubble a line to `other` uses: the one facing it, but never the right,
 * where its text stands, nor `taken`; then the left when `other` stands to its left, else the top or
 * the bottom. */
function controllerSide(controller: HmiObjectModel, other: HmiObjectModel, taken?: PortDirection): PortDirection {
  const box = bubbleBox(controller);
  const target = symbolBox(other);
  const faced = facing(box, target);
  if (faced !== "right" && faced !== taken) return faced;
  const left = target.x + target.width / 2 < box.x + box.width / 2;
  const below = target.y + target.height / 2 > box.y + box.height / 2;
  const order: PortDirection[] = left
    ? ["left", below ? "down" : "up", below ? "up" : "down"]
    : below
      ? ["down", "up", "left"]
      : ["up", "down", "left"];
  return order.find((side) => side !== taken) ?? "left";
}

function bubbleBox(object: HmiObjectModel): Box {
  const { centre, radius } = attachOrigin(object);
  return { x: centre[0] - radius, y: centre[1] - radius, width: 2 * radius, height: 2 * radius };
}

/** Where a line leaves an instrument's or a controller's bubble: on its edge, on `side`, else facing
 * `toward`. */
function bubbleEnd(object: HmiObjectModel, toward: HmiObjectModel, side?: PortDirection): PortPoint {
  const { centre, radius } = attachOrigin(object);
  const box = bubbleBox(object);
  const direction = side ?? facing(box, symbolBox(toward));
  const step = STEPS[direction];
  return { at: [centre[0] + step[0] * radius, centre[1] + step[1] * radius], direction };
}

/** Where a line meets equipment: the middle of its side that faces `from`. */
function sideEnd(object: HmiObjectModel, from: HmiObjectModel): PortPoint {
  const box = symbolBox(object);
  const { centre } = attachOrigin(from);
  const direction = facing(box, { x: centre[0], y: centre[1], width: 0, height: 0 });
  const [x, y] = [box.x + box.width / 2, box.y + box.height / 2];
  const at: Point =
    direction === "up"
      ? [x, box.y]
      : direction === "down"
        ? [x, box.y + box.height]
        : direction === "left"
          ? [box.x, y]
          : [box.x + box.width, y];
  return { at, direction };
}

const STEPS: Record<PortDirection, Point> = { left: [-1, 0], right: [1, 0], up: [0, -1], down: [0, 1] };

/** The side of `box` that faces `other`: above or below it when it stands clear of it vertically,
 * else to the left or the right. */
function facing(box: Box, other: Box): PortDirection {
  if (other.y + other.height <= box.y) return "up";
  if (other.y >= box.y + box.height) return "down";
  return other.x + other.width / 2 < box.x + box.width / 2 ? "left" : "right";
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}
