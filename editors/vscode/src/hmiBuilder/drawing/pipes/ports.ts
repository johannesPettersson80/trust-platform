// The ports of the objects on a page (specification 39, §6, item 1): where a pipe meets a symbol
// and the way it leaves it, in page pixels, for the symbol as its object places and turns it.

import type { HmiObjectModel } from "../model";
import { symbolFor, symbolPlacement } from "../symbols";

export type PortDirection = "left" | "right" | "up" | "down";

export interface PortPoint {
  at: [number, number];
  direction: PortDirection;
}

/** A junction joins pipes at its centre, one on each side. */
const JUNCTION_PORTS: Record<string, PortDirection> = { left: "left", right: "right", top: "up", bottom: "down" };

const TURNS: PortDirection[] = ["right", "down", "left", "up"];

/** A port of an object on the page; undefined when the object has no such port. */
export function portPoint(object: HmiObjectModel, port: string): PortPoint | undefined {
  const [width, height] = object.size;
  let local: PortPoint | undefined;
  if (object.type === "junction") {
    const direction = JUNCTION_PORTS[port];
    local = direction ? { at: [width / 2, height / 2], direction } : undefined;
  } else {
    const definition = symbolFor(object.type);
    const symbolPort = definition?.ports[port];
    if (definition && symbolPort) {
      const { left, top, scale } = symbolPlacement(object.size, definition.size);
      local = {
        at: [left + symbolPort.at[0] * scale, top + symbolPort.at[1] * scale],
        direction: symbolPort.direction,
      };
    }
  }
  if (!local) return undefined;
  const turned = turn(local, object.rotation ?? 0, width, height);
  return { at: [object.at[0] + turned.at[0], object.at[1] + turned.at[1]], direction: turned.direction };
}

/** A point and its direction turned with the object about its centre, in quarter turns. */
function turn(point: PortPoint, rotation: number, width: number, height: number): PortPoint {
  const quarters = ((Math.round(rotation / 90) % 4) + 4) % 4;
  if (quarters === 0) return point;
  const [cx, cy] = [width / 2, height / 2];
  let [x, y] = [point.at[0] - cx, point.at[1] - cy];
  for (let step = 0; step < quarters; step += 1) [x, y] = [-y, x];
  const direction = TURNS[(TURNS.indexOf(point.direction) + quarters) % 4];
  return { at: [cx + x, cy + y], direction };
}
