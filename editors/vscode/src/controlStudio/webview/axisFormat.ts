// Exists because ECharts calls a value-axis `axisLabel.formatter` as (value, index), so
// passing `formatEngineeringNumber` directly turned the tick's array index into its
// fraction-digit count (finding V-01: tick 0 -> "30", tick 7 -> "31.641848"). Every axis
// in Control Studio must build its formatter through this module so the bug cannot
// reappear at a new call site, and so linear axes show one shared decimal precision
// derived from their own value span instead of each tick's coincidental magnitude.

import { automaticFractionDigits, formatEngineeringNumber } from "../chartContract";

/** Wraps formatEngineeringNumber so any extra arguments a caller passes (ECharts' tick
 * index included) can never be mistaken for the fraction-digit count. */
export function axisTickFormatter(fractionDigits?: number): (value: unknown) => string {
  return (value: unknown) => formatEngineeringNumber(value, fractionDigits);
}

/** Derives one decimal-place count for every tick on a linear axis from the axis's own
 * finite value span, so nearby ticks cannot render with different precision (and a
 * near-zero floating-point artifact cannot render with far more digits than its
 * neighbours). Returns undefined for a degenerate or empty span, so the caller can fall
 * back to formatEngineeringNumber's own per-value automatic policy. */
export function linearAxisFractionDigits(values: readonly (number | null | undefined)[]): number | undefined {
  const finite = values.filter((value): value is number => typeof value === "number" && Number.isFinite(value));
  if (finite.length === 0) return undefined;
  const minimum = Math.min(...finite);
  const maximum = Math.max(...finite);
  if (minimum === maximum) return undefined;
  const magnitudeDigits = automaticFractionDigits(Math.max(Math.abs(minimum), Math.abs(maximum)));
  // Five divisions are the shared linear-axis default. A small change around
  // a large operating point still needs enough decimals to distinguish ticks.
  const spanDigits = Math.ceil(-Math.log10((maximum - minimum) / 5));
  return Math.min(6, Math.max(magnitudeDigits, spanDigits));
}

/** Keeps every tick on a log-scaled frequency axis in plain decimal notation across the
 * plotted decades, instead of switching to exponential notation only for the smallest
 * ticks — which reads as two different numbering systems on one axis (finding V-05:
 * frequency ticks mixed "1.00e-3" with "0.1"). */
export function frequencyAxisTickFormatter(): (value: unknown) => string {
  return (value: unknown) => {
    const numeric = typeof value === "number" ? value : Number(value);
    if (!Number.isFinite(numeric) || numeric <= 0) return formatEngineeringNumber(value);
    const digits = Math.max(0, Math.min(6, 3 - Math.floor(Math.log10(numeric))));
    return formatEngineeringNumber(numeric, digits);
  };
}
