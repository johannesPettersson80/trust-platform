import * as path from "path";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { BoundedAnalysisJob } from "./boundedAnalysisJob";
import type { IdentifiedStateModelResult } from "./identifiedStateModel";

/** Shared by live acquisition and opened data; this fitting boundary has no runtime client. */
export async function fitRecordedStateModel(loop: ControlLoopDefinition, runs: readonly RuntimeVerificationResult[],
  fitting: BoundedAnalysisJob, cancelled: () => boolean): Promise<IdentifiedStateModelResult> {
  if (runs.length < 1 || runs.length > 2) throw new Error("One estimation recording or two independent recordings are required.");
  const channel = (signal: ControlLoopDefinition["signals"]["processVariable"]) => ({ id: signal.symbol, unit: signal.unit, operatingRange: signal.range });
  const result = await fitting.run<IdentifiedStateModelResult>(path.join(__dirname, "generalIdentificationWorker.js"), {
    estimation: runs[0].run, validation: runs[1]?.run,
    channels: { inputs: [channel(loop.signals.manipulatedVariable)], outputs: [channel(loop.signals.processVariable)] },
    maximumNormalizedError: loop.identificationPolicy?.maximumNormalizedError ?? 0.15,
    maximumOrder: loop.identificationPolicy?.maximumOrder ?? 4,
  });
  if (!result || cancelled()) throw new Error("Identification cancelled.");
  return result;
}
