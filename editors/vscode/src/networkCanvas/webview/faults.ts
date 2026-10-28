import type { NCFault } from "./types";

export function visibleFaultsForValidationState(
  faults: readonly NCFault[],
  applyResultLocallyStale: boolean
): readonly NCFault[] {
  if (!applyResultLocallyStale) {
    return faults;
  }
  // The server marks the fault of the form's refused result (`form`); it is hidden while the form
  // is edited, and every other fault stays.
  return faults.filter((entry) => entry.form !== true);
}
