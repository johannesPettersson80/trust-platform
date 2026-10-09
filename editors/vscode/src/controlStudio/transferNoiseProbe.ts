import type { RuntimeVerificationResult } from "./verificationClient";
import type { GeneralModel } from "./generalModel";

/** The transfer trial's plant held at rest in Manual for one second, without the transfer: its measurement then
 * shows only noise, which sets the Auto-entry allowance when no identification recording measured it. */
export function restingNoiseProbeRequest<T extends object>(request: T): T {
  const stepMs = (request as { step_ms?: unknown }).step_ms;
  const step = typeof stepMs === "number" && stepMs > 0 ? stepMs : 20;
  const cycles = Math.max(10, Math.round(1000 / step));
  return { ...request, record_every: undefined, events: [], duration_ms: cycles * step, max_total_cycles: cycles + 1 };
}

/** Half of the range of a recorded signal's good samples, or undefined without at least four of them. */
export function recordedHalfRange(result: Pick<RuntimeVerificationResult, "run">, symbol: string): number | undefined {
  const values = (result.run?.samples ?? []).map(sample => sample.values[symbol])
    .filter(value => value?.quality === "good" && Number.isFinite(value.value)).map(value => value!.value as number);
  return values.length >= 4 ? (Math.max(...values) - Math.min(...values)) / 2 : undefined;
}

/** The model as seen from the configured plant's own rest, measured by the resting probe: the transfer check starts
 * the controller where that plant actually rests, which differs from a fitted operating point by any fit offset. */
export function restingOperatingModel(model: GeneralModel, resting: Pick<RuntimeVerificationResult, "run">, outputSymbols: readonly string[]): GeneralModel {
  const outputs = outputSymbols.map(symbol => {
    const values = (resting.run?.samples ?? []).map(sample => sample.values[symbol])
      .filter(value => value?.quality === "good" && Number.isFinite(value.value)).map(value => value!.value as number);
    return values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : undefined;
  });
  if (!model.operatingPoint || outputs.some(value => value === undefined)) return model;
  return { ...model, operatingPoint: { ...model.operatingPoint, outputs: outputs as number[] } };
}
