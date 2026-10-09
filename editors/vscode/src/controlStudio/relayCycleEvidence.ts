import type { IdentificationCampaignRecord } from "./automaticIdentification";
import { formatEngineeringNumber } from "./chartContract";
import { decodePidLaw } from "./controllerLaw";
import type { ControllerTuningResult } from "./controllerTuning";
import type { ControlExperimentPlan } from "./experiment";
import type { GeneralModel } from "./generalModel";
import { recordingFingerprint } from "./independentModelValidation";
import type { ControlLoopDefinition } from "./loopDefinition";
import { manualControllerCandidate } from "./manualControllerCandidate";
import { relayPiFromMeasuredRecord } from "./relayTuning";
import { requiredGainSign } from "./numerics/identification";

export type RelayCycleEvidence =
  | { status: "qualified"; cycles: number; outputAmplitude: number; responseAmplitude: number; periodSeconds: number;
      ultimateGain: number; proportionalGain: number; integralTimeSeconds: number; integralGain: number }
  | { status: "not_qualified"; reason: string };

/** The relay cycle measured in the feedback-relay recording that the selected model was fitted from. */
export function selectedModelRelayCycle(loop: ControlLoopDefinition, model: GeneralModel | undefined,
  campaigns: readonly Pick<IdentificationCampaignRecord, "runs">[]): RelayCycleEvidence | undefined {
  if (!model) return undefined;
  const record = campaigns.flatMap(campaign => campaign.runs).find(item => model.evidence.includes(recordingFingerprint(item.run)));
  const plan = record?.request.experiment as ControlExperimentPlan | undefined;
  if (!record || plan?.kind !== "relay" || !plan.relayFeedback) return undefined;
  const pv = loop.signals.processVariable.symbol, mv = loop.signals.manipulatedVariable.symbol;
  const samples = record.run.samples.flatMap(sample => {
    const processValue = sample.values[pv]?.value, output = sample.values[mv]?.value;
    return typeof processValue === "number" && typeof output === "number" ? [{ timeSeconds: sample.timeSeconds, processValue, output }] : [];
  });
  try {
    const relay = relayPiFromMeasuredRecord({ highOutput: plan.relayFeedback.highOutput, lowOutput: plan.relayFeedback.lowOutput, samples });
    if (relay.status !== "qualified") return { status: "not_qualified", reason: relay.reason };
    const { proportionalGain, integralTimeSeconds, integralGain } = relay.candidate;
    return { status: "qualified", cycles: relay.cycles.length, outputAmplitude: relay.outputAmplitude, responseAmplitude: relay.responseAmplitude,
      periodSeconds: relay.periodSeconds, ultimateGain: relay.ultimateGain, proportionalGain, integralTimeSeconds, integralGain };
  } catch (error) {
    return { status: "not_qualified", reason: error instanceof Error ? error.message : String(error) };
  }
}

/** The relay rule's PI settings, Kp = 0.45·Ku and Ti = Pu/1.2, entered into the declared PID law. */
export function relayRuleCandidate(loop: ControlLoopDefinition, model: GeneralModel, relay: RelayCycleEvidence | undefined,
  pins?: Readonly<Record<string, number>>): ControllerTuningResult {
  const pid = decodePidLaw(loop.controller.law, loop.parameterBinding);
  if (!pid) throw new Error("The relay rule needs a declared PID controller law.");
  if (relay?.status !== "qualified") throw new Error("Identify the plant with Feedback relay and repeatable cycles before using the relay rule.");
  // Ku is a magnitude; the gains take the sign of the controller action (numerical annex N5.R, N6).
  const sign = requiredGainSign(loop.controller.action ?? "direct");
  const tuned = manualControllerCandidate(loop, model, "PI",
    { [pid.proportionalGainSymbol]: sign * relay.proportionalGain, [pid.integralGainSymbol]: sign * relay.integralGain }, pins);
  return { ...tuned, reason: `Relay rule from Ku = ${formatEngineeringNumber(relay.ultimateGain)} and Pu = ${formatEngineeringNumber(relay.periodSeconds)} s: Kp = 0.45·Ku, Ti = Pu/1.2. Validate before applying.` };
}
