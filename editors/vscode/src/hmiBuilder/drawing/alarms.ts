// Alarm marks (specification 39, §13.6 items 5 and 6): each priority has its colour and its shape,
// a diamond for High, a triangle for Medium and a circle for Low, each with "!", so a priority
// never rests on colour alone.

import type { AlarmRowModel, DrawFrame, HmiObjectModel } from "./model";
import { token, TokenName } from "./parts";
import { el, SvgNode, text } from "./svg";
import { say } from "./speech";
import type { Word } from "./words";

export type AlarmPriority = "high" | "medium" | "low";

const COLOURS: Record<AlarmPriority, TokenName> = { high: "alarm-high", medium: "alarm-medium", low: "alarm-low" };
const NAMES: Record<AlarmPriority, Word> = { high: "High", medium: "Medium", low: "Low" };

/** A priority as the runtime names it, or Low for anything else. */
export function alarmPriority(value: unknown): AlarmPriority {
  return value === "high" || value === "medium" ? value : "low";
}

/** The priority's name, as lists show it. */
export function priorityName(priority: AlarmPriority): string {
  return say(NAMES[priority]);
}

const STATES: Record<string, Word> = {
  ActiveUnacknowledged: "Unacknowledged",
  ActiveAcknowledged: "Acknowledged",
  ClearedUnacknowledged: "Cleared, unacknowledged",
  Normal: "Normal",
};

/** An ISA-18.2 state as lists show it. */
export function stateName(state: string): string {
  const word = STATES[state];
  return word ? say(word) : state;
}

const HIDDEN: Record<string, Word> = { shelved: "Shelved", suppressed: "Suppressed", out_of_service: "Out of service" };

/**
 * What a row of a list's tab says about an alarm's state. A shelved, suppressed or out-of-service
 * alarm is in that state, whatever its acknowledgement; the row adds whether its condition is
 * active, which it still follows (specification 40, §3.2–3.4).
 */
export function rowStateName(row: { state: string; active: boolean }, tab: string): string {
  const hidden = HIDDEN[tab];
  return hidden
    ? say(row.active ? "{state}, condition active" : "{state}, condition clear", { state: say(hidden) })
    : stateName(row.state);
}

/** The shape and its "!", centred at (x, y) and `size` across. */
function markShapes(priority: AlarmPriority, x: number, y: number, size: number): SvgNode[] {
  const half = size / 2;
  const fill = token(COLOURS[priority]);
  const shape =
    priority === "high"
      ? el("path", { d: `M${x} ${y - half}L${x + half} ${y}L${x} ${y + half}L${x - half} ${y}Z`, fill })
      : priority === "medium"
        ? el("path", {
            d: `M${x} ${y - half}L${x + half} ${y + half * 0.8}L${x - half} ${y + half * 0.8}Z`,
            fill,
            "stroke-linejoin": "round",
          })
        : el("circle", { cx: x, cy: y, r: half * 0.84, fill });
  const drop = priority === "medium" ? size * 0.3 : size * 0.2;
  return [
    shape,
    text("!", {
      x,
      y: y + drop,
      fill: token("page"),
      "font-size": Math.round(size * 0.58),
      "font-weight": 800,
      "text-anchor": "middle",
    }),
  ];
}

/** A priority's mark in a drawing, centred at (x, y). */
export function priorityMark(priority: AlarmPriority, x: number, y: number, size: number): SvgNode {
  return el("g", { "data-part": "priority-mark", "data-priority": priority }, markShapes(priority, x, y, size));
}

/** A priority's mark as an image of its own, `size` pixels across, for HTML such as the top bar. */
export function priorityMarkImage(value: string, size: number): SvgNode {
  const priority = alarmPriority(value);
  return el(
    "svg",
    {
      xmlns: "http://www.w3.org/2000/svg",
      width: size,
      height: size,
      viewBox: `0 0 ${size} ${size}`,
      role: "img",
      "aria-label": say("{priority} priority", { priority: say(NAMES[priority]) }),
      "data-part": "priority-mark",
      "data-priority": priority,
    },
    markShapes(priority, size / 2, size / 2, size),
  );
}

/**
 * Makes a mark blink once a second (§13.6, item 5). The drawing is renewed often, so each copy
 * starts its animation at the same phase of the clock and the blink does not stutter.
 */
export function blinking(node: SvgNode, nowMs: number): SvgNode {
  node.attrs["data-blink"] = "true";
  node.attrs.class = "hmi-blink";
  node.attrs.style = `animation-delay: -${Math.floor(nowMs % 1000)}ms`;
  return node;
}

const ORDER: AlarmPriority[] = ["high", "medium", "low"];

/**
 * The annunciating alarms of an object's equipment (§13.6, item 9): those whose equipment holds
 * one of the object's bound paths. Identifiers compare without regard to case, as in IEC 61131-3.
 */
export function equipmentAlarms(object: HmiObjectModel, frame: DrawFrame): AlarmRowModel[] {
  const paths = Object.values(object.bind).map((binding) => binding.path.toLowerCase());
  return (frame.alarms?.summary?.active ?? []).filter((row) => {
    const equipment = row.equipment?.toLowerCase();
    return equipment !== undefined && paths.some((path) => path.startsWith(`${equipment}.`));
  });
}

const equipmentCountWidth = (count: number): number => 12 + String(count).length * 9;

/** Horizontal space occupied by the equipment mark and optional count. */
export function equipmentBadgeWidth(object: HmiObjectModel, frame: DrawFrame): number {
  const count = equipmentAlarms(object, frame).length;
  return count === 0 ? 0 : 30 + (count > 1 ? 4 + equipmentCountWidth(count) : 0);
}

/**
 * An object's equipment badge at its top right: the mark of the highest priority among its
 * equipment's alarms, newest first within it, with their number when there are several. It
 * blinks while one of them is unacknowledged and the data is fresh.
 */
export function equipmentBadge(object: HmiObjectModel, frame: DrawFrame): SvgNode | undefined {
  const rows = equipmentAlarms(object, frame);
  if (rows.length === 0) return undefined;
  const top = [...rows].sort(
    (a, b) =>
      ORDER.indexOf(alarmPriority(a.priority)) - ORDER.indexOf(alarmPriority(b.priority)) || b.since_ms - a.since_ms,
  )[0];
  const cx = object.size[0] - 14;
  const cy = 14;
  const mark = priorityMark(alarmPriority(top.priority), cx, cy, 22);
  if (frame.alarms?.stale !== true && rows.some((row) => row.unacknowledged)) blinking(mark, frame.nowMs ?? 0);
  const children: SvgNode[] = [el("circle", { cx, cy, r: 15, fill: token("surface"), stroke: token("border") }), mark];
  if (rows.length > 1) {
    const count = String(rows.length);
    const width = equipmentCountWidth(rows.length);
    const left = cx - 15 - 4 - width;
    children.unshift(
      el("rect", { x: left, y: cy - 11, width, height: 22, rx: 11, fill: token("raised"), stroke: token("border") }),
      text(count, {
        x: left + width / 2,
        y: cy + 5,
        fill: token("text"),
        "font-size": 14,
        "font-weight": 700,
        "text-anchor": "middle",
      }),
    );
  }
  return el("g", { "data-part": "equipment-alarm", "data-alarm": top.path, cursor: "pointer" }, children);
}
