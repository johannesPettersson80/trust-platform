import type { NetworkCanvasFault } from "../model";
import type { NCGraph } from "./types";

export function visibleFaultsForValidationState(
  faults: readonly NetworkCanvasFault[],
  applyResultLocallyStale: boolean
): readonly NetworkCanvasFault[] {
  if (!applyResultLocallyStale) {
    return faults;
  }
  return faults.filter((entry) => !String(entry.id).startsWith("apply:"));
}

/**
 * The actionable Simulator failure banner already owns the primary recovery.
 * Keep that same fault out of the header so one problem is not presented as
 * two competing errors, while preserving every unrelated fault.
 */
export function headerFaultsForBanner(
  faults: readonly NetworkCanvasFault[],
  banner: NCGraph["banner"],
): readonly NetworkCanvasFault[] {
  if (banner?.kind !== "error") {
    return faults;
  }
  const represented = new Set(banner.representedFaultIds ?? []);
  return represented.size === 0
    ? faults
    : faults.filter((fault) => !represented.has(fault.id));
}
