// Display-only thresholds (§5.9 item 3). They annotate a scale; they never infer alarm state.
import type { HmiObjectModel, LiveValue } from "./model";
import { formatValue } from "./format";
import { token } from "./parts";
import { el, SvgNode } from "./svg";

export const LIMIT_NAMES = ["low-low", "low", "high", "high-high"] as const;
export type LimitName = (typeof LIMIT_NAMES)[number];
export interface LimitTick {
  name: LimitName;
  value: number;
  fraction: number;
  priority?: "low" | "medium" | "high";
}
export interface AlarmRange {
  from: number;
  to: number;
  ticks: LimitTick[];
}

export function alarmRange(object: HmiObjectModel, live?: LiveValue): AlarmRange | undefined {
  const limits = live?.presentation ? live.presentation.limits : object.limits;
  if (!limits || !LIMIT_NAMES.some((name) => limits[name] !== undefined)) return undefined;
  const min = object.min ?? 0,
    max = object.max ?? 100;
  if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) return undefined;
  let previous = -Infinity;
  for (const name of LIMIT_NAMES) {
    const value = limits[name];
    if (value === undefined) continue;
    if (!Number.isFinite(value) || value < previous) return undefined;
    previous = value;
  }
  const fraction = (value: number): number => Math.max(0, Math.min(1, (value - min) / (max - min)));
  return {
    from: fraction(limits.low ?? limits["low-low"] ?? min),
    to: fraction(limits.high ?? limits["high-high"] ?? max),
    ticks: LIMIT_NAMES.flatMap((name) => {
      const value = limits[name];
      return value === undefined || value < min || value > max
        ? []
        : [{ name, value, fraction: fraction(value), priority: live?.presentation?.priorities?.[name] }];
    }),
  };
}

/** Every limit has its value and conventional L/LL/H/HH name as an accessible SVG title. */
export function limitTick(
  tick: LimitTick,
  coordinates: { x1: number; y1: number; x2: number; y2: number },
  object: HmiObjectModel,
): SvgNode {
  const name = { "low-low": "LL", low: "L", high: "H", "high-high": "HH" }[tick.name];
  const unit = object.unit ?? object.bind.value?.unit;
  const title = `${name} · ${formatValue(tick.value, object)}${unit ? ` ${unit}` : ""}`;
  return el(
    "line",
    {
      ...coordinates,
      "data-limit": tick.name,
      "data-fraction": tick.fraction,
      stroke: token(
        `alarm-${tick.priority ?? (tick.name === "low-low" || tick.name === "high-high" ? "high" : "medium")}`,
      ),
      "stroke-width": 2,
      "stroke-linecap": "round",
      "aria-label": title,
    },
    [{ ...el("title"), text: title }],
  );
}
