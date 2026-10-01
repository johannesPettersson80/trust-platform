import type { IdentificationCampaignRecord } from "./automaticIdentification";
import type { GeneralModel } from "./generalModel";
import { recordingFingerprint } from "./independentModelValidation";
import type { ControlLoopDefinition } from "./loopDefinition";

/** Half-range of the measured process variable over the steady baseline, before the first input change,
 * of the recording the selected model was fitted from. */
export function measuredProcessNoise(loop: ControlLoopDefinition, model: GeneralModel | undefined,
  campaigns: readonly Pick<IdentificationCampaignRecord, "runs">[]): number | undefined {
  if (!model) return undefined;
  const record = campaigns.flatMap(campaign => campaign.runs).find(item => model.evidence.includes(recordingFingerprint(item.run)));
  if (!record) return undefined;
  const pv = loop.signals.processVariable.symbol, mv = loop.signals.manipulatedVariable.symbol;
  const baseline: number[] = [];
  let held: number | undefined;
  for (const sample of record.run.samples) {
    const input = sample.values[mv]?.value, output = sample.values[pv]?.value;
    if (typeof input !== "number" || typeof output !== "number") break;
    held ??= input;
    if (Math.abs(input - held) > 1e-9 * Math.max(1, Math.abs(held))) break;
    baseline.push(output);
  }
  return baseline.length >= 10 ? (Math.max(...baseline) - Math.min(...baseline)) / 2 : undefined;
}
