import type { NetworkCanvasFault } from "../model";

export function visibleFaultsForValidationState(
  faults: readonly NetworkCanvasFault[],
  applyResultLocallyStale: boolean
): readonly NetworkCanvasFault[] {
  if (!applyResultLocallyStale) {
    return faults;
  }
  return faults.filter((entry) => !String(entry.id).startsWith("apply:"));
}
