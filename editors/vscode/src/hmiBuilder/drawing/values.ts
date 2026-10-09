// Live values for an object's bound properties.

import type { DrawFrame, HmiObjectModel, LiveValue } from "./model";

const UNKNOWN: LiveValue = { value: null, state: "unknown" };

/**
 * The live value of a bound property: the frame's value for its path, or unknown when the frame
 * has none. Undefined when the property is not bound.
 */
export function liveValueOf(object: HmiObjectModel, property: string, frame: DrawFrame): LiveValue | undefined {
  const binding = object.bind[property];
  if (!binding) return undefined;
  const value = frame.values[binding.path] ?? UNKNOWN;
  if (!binding.quality) return value;
  return withQuality(value, frame.values[binding.quality]);
}

/** Apply the explicitly mapped PLC quality without turning an invalid zero into good data. */
export function withQuality(value: LiveValue | undefined, quality: LiveValue | undefined): LiveValue {
  value = value ?? UNKNOWN;
  if (!quality || quality.state === "unknown" || quality.staleMs !== undefined) return { ...value, state: "unknown" };
  const status = String(quality.value).toLowerCase();
  if (status === "true" || status === "good") return value;
  if (status === "uncertain") return { ...value, state: "estimated" };
  return { ...value, state: "unknown" };
}

/** Whether a live value counts as on: TRUE, a non-zero number, or a name other than off. */
export function isOn(value: LiveValue["value"]): boolean {
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") return !["", "0", "false", "off"].includes(value.trim().toLowerCase());
  return false;
}
