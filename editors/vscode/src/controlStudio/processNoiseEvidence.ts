import type { IdentificationCampaignRecord } from "./automaticIdentification";
import type { GeneralModel } from "./generalModel";
import { recordingFingerprint } from "./independentModelValidation";
import type { ControlLoopDefinition } from "./loopDefinition";
import { baselineEndSeconds, experimentStartSeconds } from "./recordedExperimentStart";
import type { RuntimeVerificationResult } from "./verificationClient";

/**
 * The measured output's noise as single executions see it (Specification 35, A6.4), from the recording the selected model
 * was fitted from: three standard deviations of the measured output about its mean over the steady baseline, times √k when
 * the recording averaged k executions per sample (A4.2). The baseline is the window the recording names (B3.4 step 2), in
 * Auto or in Manual, or for a recording without one the samples before the first input change.
 */
export function measuredProcessNoise(loop: ControlLoopDefinition, model: GeneralModel | undefined,
  campaigns: readonly Pick<IdentificationCampaignRecord, "runs">[]): number | undefined {
  if (!model) return undefined;
  const record = campaigns.flatMap(campaign => campaign.runs).find(item => model.evidence.includes(recordingFingerprint(item.run)));
  if (!record) return undefined;
  const baseline = baselineSamples(loop, record);
  if (baseline.length < 10) return undefined;
  const mean = baseline.reduce((sum, sample) => sum + sample.output, 0) / baseline.length;
  const variance = baseline.reduce((sum, sample) => sum + (sample.output - mean) ** 2, 0) / (baseline.length - 1);
  return 3 * Math.sqrt(variance) * Math.sqrt(executionsPerSample(loop, baseline.map(sample => sample.timeSeconds)));
}

interface BaselineSample { timeSeconds: number; output: number }

function baselineSamples(loop: ControlLoopDefinition, record: RuntimeVerificationResult): BaselineSample[] {
  const pv = loop.signals.processVariable.symbol, mv = loop.signals.manipulatedVariable.symbol;
  const samples: BaselineSample[] = [];
  const end = baselineEndSeconds(record);
  if (end !== undefined) {
    const start = experimentStartSeconds(record);
    for (const sample of record.run.samples) {
      if (sample.timeSeconds > end + 1e-9) break;
      const output = sample.values[pv]?.value;
      if (sample.timeSeconds >= start - 1e-9 && typeof output === "number") samples.push({ timeSeconds: sample.timeSeconds, output });
    }
    return samples;
  }
  let held: number | undefined;
  for (const sample of record.run.samples) {
    const input = sample.values[mv]?.value, output = sample.values[pv]?.value;
    if (typeof input !== "number" || typeof output !== "number") break;
    held ??= input;
    if (Math.abs(input - held) > 1e-9 * Math.max(1, Math.abs(held))) break;
    samples.push({ timeSeconds: sample.timeSeconds, output });
  }
  return samples;
}

/** The executions one recorded sample averages: the median sample spacing over the loop's task period (A4.2, A4.3). */
function executionsPerSample(loop: ControlLoopDefinition, times: readonly number[]): number {
  const spacings = times.slice(1).map((time, index) => time - times[index]).sort((left, right) => left - right);
  const median = spacings[Math.floor(spacings.length / 2)] ?? 0;
  return Math.max(1, Math.round(median * 1000 / loop.controller.taskPeriodMs));
}
