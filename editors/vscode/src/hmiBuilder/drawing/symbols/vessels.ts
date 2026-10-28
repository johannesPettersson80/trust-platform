// Open vessels (ISO 10628-2): walls open at the top with lips at the rim, the bottom corners
// rounded, and the liquid filled from the bottom to its level in the accent of live values
// (specification 39, §5.3 item 5, fill a level).

import { el, SvgNode } from "../svg";
import { BODY, HATCH, LINE, LIQUID, round } from "./style";

/** A vessel's walls from its rim at `top` down to `bottom`, between `left` and `right`. */
export interface Walls {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

/** The walls' outline: up one side, along the rounded bottom and up the other, open at the top. */
function outline({ left, right, top, bottom }: Walls): string {
  return `M${left} ${top} V${bottom - 3} Q${left} ${bottom} ${left + 3} ${bottom} H${right - 3} Q${right} ${bottom} ${right} ${bottom - 3} V${top}`;
}

/** The vessel's outline for an attached instrument (§21.6): its walls, closed across the rim. */
export function vesselOutline(walls: Walls): string {
  return `${outline(walls)} Z`;
}

/** A rectangle with rounded corners, as SVG draws a `rect` with `rx`. */
export interface RoundedRect {
  x: number;
  y: number;
  width: number;
  height: number;
  rx: number;
}

/** A rounded rectangle's outline as path data, its corners the quarter circles SVG draws (§21.6). */
export function roundedRectOutline({ x, y, width, height, rx }: RoundedRect): string {
  const [right, bottom] = [x + width, y + height];
  const corner = (toX: number, toY: number): string => `A${rx} ${rx} 0 0 1 ${toX} ${toY}`;
  return `M${x + rx} ${y} H${right - rx} ${corner(right, y + rx)} V${bottom - rx} ${corner(right - rx, bottom)} H${x + rx} ${corner(x, bottom - rx)} V${y + rx} ${corner(x + rx, y)} Z`;
}

/**
 * The vessel: its body, the liquid at `level` (0 to 1; none when undefined), then the walls and
 * lips over the liquid's edges. An unknown state hatches the body and draws no liquid.
 */
export function openVessel(walls: Walls, part: string, level: number | undefined, unknown: boolean): SvgNode[] {
  const { left, right, top, bottom } = walls;
  const inside = { x: left + 1, y: top + 1, width: right - left - 2, height: bottom - top - 2 };
  const nodes: SvgNode[] = [
    el("path", { "data-part": "body", d: outline(walls), fill: unknown ? HATCH : BODY, stroke: "none" }),
  ];
  if (!unknown && level !== undefined) {
    const height = round(inside.height * level);
    nodes.push(
      el("rect", {
        "data-part": part,
        "data-level": round(level),
        x: inside.x,
        y: round(inside.y + inside.height - height),
        width: inside.width,
        height,
        fill: LIQUID,
      }),
    );
  }
  nodes.push(
    el("path", {
      "data-part": "walls",
      d: `${outline(walls)} M${left - 4} ${top} H${left} M${right} ${top} H${right + 4}`,
      fill: "none",
      ...LINE,
    }),
  );
  return nodes;
}

/** The liquid in a vessel whose bottom corners are rounded by `radius`: from the bottom of `box`
 * up to `level`, 0 to 1. */
export function roundedLiquid(
  part: string,
  box: { x: number; y: number; width: number; height: number },
  level: number | undefined,
  radius: number,
): SvgNode[] {
  if (level === undefined || level <= 0) return [];
  const height = round(box.height * level);
  const top = round(box.y + box.height - height);
  const bottom = box.y + box.height;
  const r = Math.min(radius, height);
  const [left, right] = [box.x, box.x + box.width];
  const d = `M${left} ${top} H${right} V${round(bottom - r)} Q${right} ${bottom} ${round(right - r)} ${bottom} H${round(left + r)} Q${left} ${bottom} ${left} ${round(bottom - r)} Z`;
  return [el("path", { "data-part": part, "data-level": round(level), d, fill: LIQUID })];
}
