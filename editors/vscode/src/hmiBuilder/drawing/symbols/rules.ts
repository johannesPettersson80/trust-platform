// Visualization rules (specification 39, §5.3 item 5): each maps a binding to a part of a symbol:
// turn it, move it to and fro, show it working, stand it at an angle, fill its level, move it
// along its travel, blink it while an alarm is unacknowledged, show a value in it, paint it in the
// alarm colour, or show it only while a number equals a value (a state among several). A symbol
// declares its rules and draws its parts from what they give. Turning and blinking show a
// current observation only (§5.3 item 3).

import type { DrawFrame, HmiObjectModel, LiveValue } from "../model";
import { observed } from "./motion";
import { on, shownBoundNumber, turning, ValueOf } from "./reading";

/** One rule: the part, and the binding that drives it in one of these ways. */
export type Rule = { part: string } & (
  | { spin: string }
  | { stroke: string }
  | { angle: string }
  | { level: string; range?: [number, number] }
  | { travel: string; range?: [number, number]; move?: [number, number] }
  | { blink: string }
  | { show: string }
  | { alarm: string }
  | { active: string }
  | { equals: string; value: number }
);

/** What the rules give a part. */
export interface PartView {
  /** It turns, with the delay that continues its turn. */
  spin?: { delayMs: number };
  /** It moves to and fro, as a piston, with the delay that continues its stroke. */
  stroke?: { delayMs: number };
  /** It stands at this angle, in degrees. */
  angle?: number;
  /** It is filled to this share of its height, 0 to 1. */
  level?: number;
  /** It stands this far along its travel, 0 to 1. */
  travel?: number;
  /** For a part with its travel's displacement: how far it is moved, in the drawing's units. */
  moved?: [number, number];
  /** It is not shown: the number it follows does not equal its value. */
  hidden?: boolean;
  /** It blinks. */
  blink?: boolean;
  /** The text it shows. */
  text?: string;
  /** It is painted in the alarm colour. */
  alarm?: boolean;
  /** It works: a heater's element heats, heat leaves. */
  active?: boolean;
}

/** What the rules give each part, from the bindings' values; an unbound binding gives nothing. */
export function applyRules(
  rules: Rule[],
  value: ValueOf,
  frame: DrawFrame,
  object: HmiObjectModel,
): Record<string, PartView> {
  const parts: Record<string, PartView> = {};
  const set = (part: string, view: PartView): void => {
    parts[part] = { ...parts[part], ...view };
  };
  for (const rule of rules) {
    if ("spin" in rule) {
      const spin = turning(value(rule.spin), frame);
      if (spin) set(rule.part, { spin });
    } else if ("stroke" in rule) {
      const stroke = turning(value(rule.stroke), frame);
      if (stroke) set(rule.part, { stroke });
    } else if ("angle" in rule) {
      const angle = number(value(rule.angle));
      if (angle !== undefined) set(rule.part, { angle });
    } else if ("level" in rule) {
      const level = share(value(rule.level), rule.range ?? range(object));
      if (level !== undefined) set(rule.part, { level });
    } else if ("travel" in rule) {
      const travel = share(value(rule.travel), rule.range ?? range(object));
      if (travel !== undefined) {
        const moved = rule.move ? ([travel * rule.move[0], travel * rule.move[1]] as [number, number]) : undefined;
        set(rule.part, moved ? { travel, moved } : { travel });
      }
    } else if ("blink" in rule) {
      const alarm = value(rule.blink);
      if (on(alarm) && observed(alarm)) set(rule.part, { blink: true });
    } else if ("show" in rule) {
      const text = shownBoundNumber(value, rule.show, frame, object);
      if (text !== undefined) set(rule.part, { text });
    } else if ("alarm" in rule) {
      if (on(value(rule.alarm))) set(rule.part, { alarm: true });
    } else if ("equals" in rule) {
      // A state's picture: shown while its number is known and equals the value.
      const shown = number(value(rule.equals));
      set(rule.part, { hidden: shown === undefined || shown !== rule.value });
    } else if ("active" in rule) {
      const bound = value(rule.active);
      const working =
        bound !== undefined &&
        bound.state !== "unknown" &&
        (typeof bound.value === "boolean" ? bound.value : Number(bound.value) > 0);
      if (working) set(rule.part, { active: true });
    }
  }
  return parts;
}

function number(value: LiveValue | undefined): number | undefined {
  const parsed = Number(value?.value);
  return value === undefined || value.state === "unknown" || value.value === null || !Number.isFinite(parsed)
    ? undefined
    : parsed;
}

/** The object's range, 0 to 100 where it sets none. */
function range(object: HmiObjectModel): [number, number] {
  return [object.min ?? 0, object.max ?? 100];
}

/** A number's share of a range, kept within 0 and 1. */
function share(value: LiveValue | undefined, [min, max]: [number, number]): number | undefined {
  const parsed = number(value);
  if (parsed === undefined || max === min) return undefined;
  return Math.min(1, Math.max(0, (parsed - min) / (max - min)));
}
