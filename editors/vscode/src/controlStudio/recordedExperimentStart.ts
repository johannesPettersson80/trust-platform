import type { RuntimeVerificationResult } from "./verificationClient";

/**
 * Where a recording's experiment data begins, in its own seconds. A connected identification records the wait for a steady
 * baseline too; its record names the start of the steady baseline window (`baselineStartSeconds`, B3.4). The wait before it
 * stays in the recording and its evidence, but fits and noise estimates never use it. Other recordings begin with their data.
 */
export function experimentStartSeconds(result: RuntimeVerificationResult | undefined): number {
  const value = (result?.request as { baselineStartSeconds?: unknown } | undefined)?.baselineStartSeconds;
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : 0;
}

/** Where a connected identification's steady baseline window ends (`baselineEndSeconds`, B3.4), or undefined for other recordings. */
export function baselineEndSeconds(result: RuntimeVerificationResult | undefined): number | undefined {
  const value = (result?.request as { baselineEndSeconds?: unknown } | undefined)?.baselineEndSeconds;
  return typeof value === "number" && Number.isFinite(value) && value > experimentStartSeconds(result) ? value : undefined;
}
