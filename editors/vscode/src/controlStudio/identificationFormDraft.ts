import type { IdentificationModelSelection } from "./identification";

export const IDENTIFICATION_FORM_KEYS = [
  "baselineDurationSeconds", "finalDurationSeconds", "maximumWindowRange", "maximumWindowSlopePerSecond",
  "measurementBound", "onsetThreshold", "onsetSamples", "baselineStep", "gainStep", "timestampShiftSeconds",
  "delay.minimum", "delay.maximum", "delay.step", "timeConstant.minimum", "timeConstant.maximum", "timeConstant.step",
] as const;
export type IdentificationFormKey = typeof IDENTIFICATION_FORM_KEYS[number];
export const IDENTIFICATION_COMPARISON_FORM_KEYS = ["sensorTimeConstant.minimum", "sensorTimeConstant.maximum", "sensorTimeConstant.step",
  "modelAdequacy.maximumAbsoluteResidual", "modelAdequacy.maximumFinalError"] as const;
export const ALL_IDENTIFICATION_FORM_KEYS = [...IDENTIFICATION_FORM_KEYS, ...IDENTIFICATION_COMPARISON_FORM_KEYS] as const;
export interface IdentificationFormDraft {
  schemaVersion: 1;
  model: IdentificationModelSelection;
  heldStep: boolean;
  values: Readonly<Record<IdentificationFormKey, string> & Partial<Record<typeof IDENTIFICATION_COMPARISON_FORM_KEYS[number], string>>>;
  excludedStart: string;
  excludedEnd: string;
}

export function decodeIdentificationFormDraft(value: unknown): IdentificationFormDraft | undefined {
  if (!record(value) || value.schemaVersion !== 1 || !["automatic", "integrating", "first_order", "fopdt", "second_order", "sopdt"].includes(String(value.model)) ||
      typeof value.heldStep !== "boolean" || typeof value.excludedStart !== "string" || typeof value.excludedEnd !== "string" ||
      !record(value.values) || Object.keys(value).some(key => !["schemaVersion", "model", "heldStep", "values", "excludedStart", "excludedEnd"].includes(key)) ||
      Object.keys(value.values).some(key => !(ALL_IDENTIFICATION_FORM_KEYS as readonly string[]).includes(key)
        || typeof value.values[key] !== "string" || value.values[key].length > 256) ||
      IDENTIFICATION_FORM_KEYS.some(key => typeof value.values[key] !== "string" || value.values[key].length > 256) ||
      value.excludedStart.length > 256 || value.excludedEnd.length > 256) return undefined;
  return structuredClone(value) as IdentificationFormDraft;
}
function record(value: unknown): value is Record<string, any> { return !!value && typeof value === "object" && !Array.isArray(value); }
