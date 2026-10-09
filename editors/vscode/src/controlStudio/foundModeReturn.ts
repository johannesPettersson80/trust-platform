import type { RuntimeTransferClient, TransferStatus, TransferTracking } from "./runtimeTransfer";

/** The mode an operation found the loop in (A6.2): a loop found in Auto returns to Auto, one found in Manual stays in Manual. */
export type FoundMode = "auto" | "manual";

/** The found mode of a review. Undefined when the review did not read the mode, so no return to it is claimed. */
export function foundModeOf(reviewedMode: boolean | undefined, manualValue: boolean): FoundMode | undefined {
  return reviewedMode === undefined ? undefined : reviewedMode === manualValue ? "manual" : "auto";
}

/** The mode value a found mode stands for, as a recovery reviewed from Apply's report sets it; unknown when none is named. */
export function reviewedModeOf(foundMode: FoundMode | undefined, manualValue: boolean): boolean | undefined {
  return foundMode === undefined ? undefined : foundMode === "manual" ? manualValue : !manualValue;
}

/** The Auto entry that returns a recovered loop to Auto: the states it writes, its transfer tolerance and its tracking (A6.3). */
export interface AutoReentry {
  readonly values: Record<string, number>;
  readonly tolerance: number;
  readonly tracking: TransferTracking;
}

/**
 * A6.2: once the previous settings are confirmed back, a loop found in Auto re-enters Auto with them. A loop found in Manual, or of
 * unknown mode, stays in Manual, and so does a loop whose Auto entry the caller cannot build (`entry` returns undefined). Returns
 * whether the loop re-entered Auto.
 */
export async function reenterFoundMode(transfer: RuntimeTransferClient, foundMode: FoundMode | undefined,
  entry: (current: TransferStatus) => AutoReentry | undefined): Promise<boolean> {
  if (foundMode !== "auto") return false;
  const current = await transfer.status();
  const { binding } = transfer;
  if (current.values[binding.mode.symbol] !== binding.mode.manual_value) return false;
  const reentry = entry(current);
  if (!reentry) return false;
  await transfer.apply(reentry.values, false, reentry.tolerance, reentry.tracking, current);
  return true;
}
