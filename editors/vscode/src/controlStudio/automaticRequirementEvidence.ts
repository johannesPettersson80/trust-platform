import type { RuntimeVerificationResult } from "./verificationClient";
import { experimentStartSeconds } from "./recordedExperimentStart";

export function recordedValues(record: RuntimeVerificationResult | undefined, symbol: string): number[] {
  return record?.run.samples.flatMap(sample => {
    const value = sample.values[symbol];
    return value?.quality === "good" && value.value !== null && Number.isFinite(value.value) ? [value.value] : [];
  }) ?? [];
}

/** Noise comes only from a complete pre-excitation baseline, never from the plant transient or the wait before the steady baseline
 * window (B3.4). */
export function baselineNoise(record: RuntimeVerificationResult | undefined, inputSymbols: readonly string[], outputSymbol: string): number {
  if (!record) return 0;
  const start = experimentStartSeconds(record);
  const samples = record.run.samples.filter(sample => sample.timeSeconds >= start - 1e-9);
  if (samples.length < 8) return 0;
  const first = samples[0];
  const changed = samples.findIndex(sample => inputSymbols.some(symbol => {
    const start = first.values[symbol], current = sample.values[symbol];
    return start?.quality !== "good" || current?.quality !== "good" || start.value === null || current.value === null
      || Math.abs(current.value - start.value) > Math.max(1e-9, Math.abs(start.value) * 1e-7);
  }));
  if (changed < 8) return 0;
  const baseline = samples.slice(0, changed);
  const values = baseline.map(sample => sample.values[outputSymbol]);
  if (values.some(value => value?.quality !== "good" || value.value === null || !Number.isFinite(value.value))) return 0;
  const ys = values.map(value => value!.value!);
  const meanX = (ys.length - 1) / 2, meanY = ys.reduce((a, b) => a + b, 0) / ys.length;
  const slope = ys.reduce((sum, y, i) => sum + (i - meanX) * (y - meanY), 0)
    / ys.reduce((sum, _, i) => sum + (i - meanX) ** 2, 0);
  return 3 * Math.sqrt(ys.reduce((sum, y, i) => sum + (y - meanY - slope * (i - meanX)) ** 2, 0) / Math.max(1, ys.length - 2));
}
