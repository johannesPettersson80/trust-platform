import { HmiAlarmResult, HmiSchemaResult, HmiTrendResult, HmiValuesResult } from "./types";

export function isRecord(value: unknown): value is Record<string, any> {
  return !!value && typeof value === "object";
}

export function isHmiSchemaResult(value: unknown): value is HmiSchemaResult {
  if (!isRecord(value)) {
    return false;
  }
  return (
    typeof value.version === "number" &&
    Array.isArray(value.pages) &&
    Array.isArray(value.widgets)
  );
}

export function isHmiValuesResult(value: unknown): value is HmiValuesResult {
  return isRecord(value) && typeof value.connected === "boolean" && isRecord(value.values);
}

export function isHmiTrendResult(value: unknown): value is HmiTrendResult {
  return (
    isRecord(value) &&
    typeof value.connected === "boolean" &&
    typeof value.duration_ms === "number" &&
    Array.isArray(value.series)
  );
}

export function isHmiAlarmResult(value: unknown): value is HmiAlarmResult {
  return (
    isRecord(value) &&
    typeof value.connected === "boolean" &&
    Array.isArray(value.active) &&
    Array.isArray(value.history)
  );
}
