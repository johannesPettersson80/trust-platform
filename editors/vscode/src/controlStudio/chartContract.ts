import { displayEngineeringUnit } from "./unitDisplay";

export { displayEngineeringUnit };

/** Keeps the live SP/PV trace off the plot frame edge instead of autoscaling tight to the
 * current data, which reads as a trace "pinned to the ceiling" on the first live plot a
 * student sees (finding V-08). Returns undefined when there is no finite data yet, so a
 * caller can leave the axis on its default range rather than forcing a degenerate one. */
export function liveResponseAxisRange(values: readonly (number | null | undefined)[]): {
  minimum: number;
  maximum: number;
} | undefined {
  const finite = values.filter((value): value is number => typeof value === "number" && Number.isFinite(value));
  if (finite.length === 0) return undefined;
  const minimum = Math.min(...finite);
  const maximum = Math.max(...finite);
  const span = maximum - minimum;
  const magnitude = Math.max(Math.abs(minimum), Math.abs(maximum));
  // Subtracting two sensor values can leave a few ulps around a constant
  // difference. Frame that as a constant; keep every recorded value unchanged.
  const varying = span > 128 * Number.EPSILON * magnitude;
  const padding = varying ? span * 0.08 : magnitude === 0 ? 1 : magnitude * 0.05;
  return { minimum: minimum - padding, maximum: maximum + padding };
}

/** Keeps process values readable without changing their full-precision source. */
export function formatEngineeringNumber(value: unknown, fractionDigits?: number): string {
  if (value === "unbounded") return "∞";
  const numeric = typeof value === "number"
    ? value
    : typeof value === "string" && value.trim() !== "" ? Number(value) : Number.NaN;
  if (numeric === Number.POSITIVE_INFINITY) return "∞";
  if (numeric === Number.NEGATIVE_INFINITY) return "−∞";
  if (!Number.isFinite(numeric)) return "—";
  if (Object.is(numeric, -0) || numeric === 0) return "0";
  const digits = fractionDigits === undefined
    ? automaticFractionDigits(Math.abs(numeric))
    : Math.max(0, Math.min(6, Math.floor(fractionDigits)));
  if (fractionDigits === undefined && Math.abs(numeric) < 0.001) return numeric.toExponential(2);
  if (Math.abs(numeric) < 10 ** -digits) return numeric.toExponential(2);
  return numeric.toFixed(digits).replace(/(\.\d*?[1-9])0+$|\.0+$/u, "$1");
}

/** The same bucket of decimal places `formatEngineeringNumber` picks automatically for a
 * single value, exposed so an axis formatter can derive one shared digit count from an
 * axis's overall span instead of from each tick's coincidental magnitude. */
export function automaticFractionDigits(magnitude: number): number {
  if (magnitude >= 1_000) return 0;
  if (magnitude >= 100) return 1;
  if (magnitude >= 1) return 2;
  if (magnitude >= 0.01) return 4;
  return 6;
}
