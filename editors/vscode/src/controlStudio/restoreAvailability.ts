import { sameParameterHistory, type ParameterStatus } from "./parameterApplication";

/**
 * Restore previous settings (Specification 35, B6.4) returns the settings Apply wrote only while they are still the active
 * ones. Once the program, the active revision or the values changed since Apply, Restore is unavailable and nothing is
 * overwritten. Returns the reason, or undefined when Restore may proceed.
 */
export function restoreUnavailableReason(observed: ParameterStatus, applied: ParameterStatus | undefined): string | undefined {
  if (!applied || sameParameterHistory(observed, applied)) return undefined;
  return "Restore is unavailable: the controller settings or program changed after Apply, so restoring would overwrite another change. Nothing was written.";
}
