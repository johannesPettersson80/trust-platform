import type { ControlPidParameters } from "./controllerCandidate";

const NUMERIC_KEYS = [
  "proportionalGain", "integralGain", "derivativeGain", "proportionalSetpointWeight",
  "derivativeSetpointWeight", "bias", "outputMinimum", "outputMaximum", "rateMinimum",
  "rateMaximum", "trackingTime",
] as const;

export type ControlPidSettingsValidation =
  | { readonly ok: true; readonly value: ControlPidParameters }
  | { readonly ok: false; readonly reason: string };

/** Validates the complete Control.PID record before a Manual or Imported tune is posted. */
export function validateControlPidSettings(value: unknown): ControlPidSettingsValidation {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return { ok: false, reason: "Control.PID settings must be a JSON object." };
  }
  const record = value as Record<string, unknown>;
  const expected = [...NUMERIC_KEYS, "antiWindup"].sort();
  if (Object.keys(record).sort().join(",") !== expected.join(",")) {
    return { ok: false, reason: "Control.PID settings must contain every required field exactly once." };
  }
  for (const key of NUMERIC_KEYS) {
    if (typeof record[key] !== "number" || !Number.isFinite(record[key])) {
      return { ok: false, reason: `Control.PID ${key} must be a finite number.` };
    }
  }
  if (record.antiWindup !== "BackCalculation" && record.antiWindup !== "Combined") {
    return { ok: false, reason: "Control.PID anti-windup must be BackCalculation or Combined." };
  }
  const outputMinimum = record.outputMinimum as number;
  const outputMaximum = record.outputMaximum as number;
  const rateMinimum = record.rateMinimum as number;
  const rateMaximum = record.rateMaximum as number;
  const trackingTime = record.trackingTime as number;
  if (outputMinimum > outputMaximum) {
    return { ok: false, reason: "Control.PID output minimum must not exceed output maximum." };
  }
  if (rateMinimum > rateMaximum) {
    return { ok: false, reason: "Control.PID rate minimum must not exceed rate maximum." };
  }
  if (trackingTime <= 0) {
    return { ok: false, reason: "Control.PID tracking time must be positive." };
  }
  return { ok: true, value: record as unknown as ControlPidParameters };
}
