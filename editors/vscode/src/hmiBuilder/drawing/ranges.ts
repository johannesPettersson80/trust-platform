// A value read against an object's display range (min..max, by default 0..100).

import type { HmiObjectModel, LiveValue } from "./model";

export interface RangeReading {
  /** The drawn share of the range, 0 to 1. */
  fraction: number;
  /** Set when the value lies outside the range and the drawing is clamped. */
  range?: "over" | "under";
}

/** The reading of a live value, or undefined when it has no number to draw. */
export function rangeReading(live: LiveValue | undefined, object: HmiObjectModel): RangeReading | undefined {
  if (!live || live.state === "unknown" || typeof live.value !== "number" || !Number.isFinite(live.value)) {
    return undefined;
  }
  const min = object.min ?? 0;
  const max = object.max ?? 100;
  const raw = max === min ? 0 : (live.value - min) / (max - min);
  if (raw > 1) return { fraction: 1, range: "over" };
  if (raw < 0) return { fraction: 0, range: "under" };
  return { fraction: Number(raw.toFixed(6)) };
}
