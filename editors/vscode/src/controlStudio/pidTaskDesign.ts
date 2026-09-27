import type { GeneralModel } from "./generalModel";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { linearTransferModel } from "./linearTransferModel";
import { designTypedPiCandidate, designTypedDerivativeCandidate, type PidPiDesign } from "./pidPiDesign";
import { MainWorkflowError } from "./mainWorkflowFailure";

/** Reuse qualified first-order rules; coefficient matching covers delay-free low-order motion/unstable plants. */
export function designPidForTask(model: GeneralModel, family: "PI" | "PD" | "PID", task: EngineerRequirements, periodMs: number): PidPiDesign {
  try {
    const options = { method: task.tuningMethod ?? "SIMC" as const, objective: task.tuningObjective ?? "balanced" as const };
    return family === "PI" ? designTypedPiCandidate(model, options) : designTypedDerivativeCandidate(model, family, options);
  } catch (error) {
    if (task.tuningMethod !== undefined) throw new MainWorkflowError(
      `${error instanceof Error ? error.message : String(error)} Choose Automatic or another tuning rule.`,
      { cause: "settings", nextAction: "retune" });
    const transfer = linearTransferModel(model);
    if (transfer.delaySeconds > 0) throw error;
    return poleMatchedPid(model, family, task, periodMs);
  }
}
export function poleMatchedPid(model: GeneralModel, family: "P" | "PI" | "PD" | "PID", task: EngineerRequirements, periodMs: number): PidPiDesign {
  const plant = linearTransferModel(model);
  if (plant.delaySeconds !== 0 || plant.numerator.length !== 1 || ![2, 3].includes(plant.denominator.length)
    || plant.numerator[0] === 0 || plant.denominator[0] <= 0) throw new Error("Pole matching needs a delay-free first/second-order model without zeros; use a compatible declared native tuning policy for other structures.");
  const b = plant.numerator[0], [leading] = plant.denominator;
  const damping = 0.8, omega = 4 / (damping * task.settlingTimeSeconds);
  if (omega * periodMs / 1000 >= 0.2) throw new Error("The requested bandwidth needs a faster sample period or a longer response time.");
  let proportionalGain = 0, integralGain = 0, derivativeGain = 0;
  const a0 = plant.denominator[plant.denominator.length - 1];
  const allowance = task.maximumError - task.settlingBand;
  const numeratorError = Math.max(Math.abs(a0 * task.referenceChange + b * (model.operatingPoint?.inputs[0] ?? 0)),
    Math.abs(b * ((model.operatingPoint?.inputs[0] ?? 0) - (task.constantInputDisturbance ?? 0))));
  if (plant.denominator.length === 2) {
    if (family === "P") {
      const decay = Math.max(4 / task.settlingTimeSeconds, numeratorError / allowance / leading);
      proportionalGain = (leading * decay - a0) / b;
    } else if (family === "PI" || family === "PID") {
      proportionalGain = (2 * damping * omega * leading - a0) / b;
      integralGain = leading * omega * omega / b;
      if (family === "PID") throw new Error("A derivative term is not justified by this first-order pole-matching rule; choose PI or the declared native tuning policy.");
    } else throw new Error("Choose P/PI or a declared derivative tuning policy for this first-order plant.");
  } else {
    const a1 = plant.denominator[1];
    if (family === "P") {
      if (a1 <= 0 || a1 / (2 * leading) < 4 / task.settlingTimeSeconds) throw new Error("P cannot provide the required damping; compare PD or PID.");
      proportionalGain = (Math.max(leading * omega * omega, numeratorError / allowance) - a0) / b;
    } else if (family === "PD") {
      proportionalGain = (Math.max(leading * omega * omega, numeratorError / allowance) - a0) / b;
      const realizedOmega = Math.sqrt((a0 + b * proportionalGain) / leading);
      derivativeGain = (2 * damping * realizedOmega * leading - a1) / b;
    } else if (family === "PID") {
      const extraPole = 3 * omega;
      derivativeGain = (leading * (2 * damping * omega + extraPole) - a1) / b;
      proportionalGain = (leading * (omega * omega + 2 * damping * omega * extraPole) - a0) / b;
      integralGain = leading * omega * omega * extraPole / b;
    } else {
      if (a1 <= 0) throw new Error("PI cannot supply the missing second-order damping; compare PD or PID.");
      const achievableOmega = a1 / leading / (2 * damping + 3);
      if (achievableOmega < omega) throw new Error("PI's fixed damping cannot meet the requested response time; compare PD/PID.");
      proportionalGain = (leading * achievableOmega * achievableOmega * (1 + 6 * damping) - a0) / b;
      integralGain = 3 * leading * achievableOmega ** 3 / b;
    }
  }
  if (![proportionalGain, integralGain, derivativeGain].every(Number.isFinite)) throw new Error("Pole-matched settings are nonfinite.");
  const denominator = integralGain !== 0
    ? plant.denominator.length === 2 ? [leading + b * derivativeGain, a0 + b * proportionalGain, b * integralGain]
      : [leading, plant.denominator[1] + b * derivativeGain, a0 + b * proportionalGain, b * integralGain]
    : plant.denominator.length === 2 ? [leading + b * derivativeGain, a0 + b * proportionalGain]
      : [leading, plant.denominator[1] + b * derivativeGain, a0 + b * proportionalGain];
  if (denominator.some(value => value <= 0) || denominator.length === 4 && denominator[1] * denominator[2] <= denominator[0] * denominator[3] * (1 + 1e-10)) throw new Error("The coefficient-matched controller does not have a strictly stable continuous characteristic polynomial.");
  return { status: "provisional", candidate: { applied: false, structure: family, proportionalGain, integralGain, derivativeGain },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` }, actualSTVerify: true,
    scope: "Continuous coefficient-matched candidate with an explicit sample-bandwidth restriction. Actual ST, filtering, limits, load and target scenarios decide qualification." };
}
