import { formatEngineeringNumber } from "./chartContract";
import { engineerPrediction } from "./engineerPrediction";
import { responseDeadlineSeconds, type EngineerRequirements } from "./engineerVerificationPlan";
import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import { MainWorkflowError } from "./mainWorkflowFailure";
import { designTypedProportionalCandidate, type PidPiDesign } from "./pidPiDesign";
import { designPidForTask, poleMatchedPid } from "./pidTaskDesign";
import { displayEngineeringUnit } from "./unitDisplay";
import { decodePidLaw } from "./controllerLaw";
import { linearTransferModel } from "./linearTransferModel";

type Response = NonNullable<EngineerRequirements["tuningObjective"]>;

/** From the declared response outward; of two equally near responses the more robust one first. */
const NEAREST: Record<Response, readonly Response[]> = {
  robust: ["robust", "balanced", "fast"], balanced: ["balanced", "robust", "fast"], fast: ["fast", "balanced", "robust"],
};

/**
 * The Automatic tuning method (Specification 35 §9.2): the response nearest the declared one whose prediction on the
 * selected model meets the declared requirements. SIMC or AMIGO selected explicitly keeps its formula.
 */
export function designPidToRequirements(loop: ControlLoopDefinition, model: GeneralModel, family: "PI" | "PD" | "PID",
  task: EngineerRequirements): PidPiDesign {
  const design = (tuningObjective: Response) => designPidForTask(model, family, { ...task, tuningObjective }, loop.controller.taskPeriodMs);
  if (task.tuningMethod !== undefined) return designPidForTask(model, family, task, loop.controller.taskPeriodMs);
  const shortfalls: Partial<Record<Response, string>> = {};
  for (const [index, response] of NEAREST[task.tuningObjective ?? "balanced"].entries()) {
    const candidate = design(response);
    const outcome = predictedShortfall(loop, model, family, candidate, task);
    // Only the declared response may go on to Validate unpredicted; an alternative must be shown to meet the task.
    if (outcome === "meets" || outcome === "unpredicted" && index === 0) return candidate;
    if (outcome !== "unpredicted") shortfalls[response] = outcome;
  }
  // No formula response settles in time: place the poles so the envelope reaches the settling band by the settling time,
  // then faster while the prediction, which includes the controller's own zeros, still misses.
  let placedShortfall: string | undefined;
  for (let placement = 0, envelope = Math.max(4, Math.log(Math.abs(task.referenceChange) / task.settlingBand)); placement < 8;
    placement++, envelope *= 1.25) {
    let placed: PidPiDesign;
    try { placed = poleMatchedPid(model, family, task, loop.controller.taskPeriodMs, envelope); }
    catch (error) {
      // Without a pole-placement form, or beyond the sample-rate limit, the formula responses' refusal stands.
      if (/Pole matching needs|bandwidth/.test(error instanceof Error ? error.message : String(error))) break;
      continue;
    }
    const { proportionalGain, integralGain, derivativeGain } = placed.candidate;
    if ([proportionalGain, integralGain, derivativeGain].some(gain => (gain ?? 0) < 0)) continue;
    const outcome = predictedShortfall(loop, model, family, placed, task);
    if (outcome === "meets") return placed;
    if (outcome !== "unpredicted") placedShortfall = outcome;
  }
  // The fastest placement settles in time when a formula response did not, so what it still misses is what decides.
  const reason = placedShortfall ? `even its fastest response ${placedShortfall}` : shortfalls.fast ? `even its fastest response ${shortfalls.fast}`
    : `its predicted response ${shortfalls.balanced ?? shortfalls.robust}`;
  throw new MainWorkflowError(`${family} cannot meet the requirements on the model: ${reason}`, { cause: "task-infeasible", nextAction: "revise-requirements" });
}

/**
 * What the predicted response misses: Validate's allowed error from the deadline to the end of the test, and the
 * settling band around its final value.
 */
function predictedShortfall(loop: ControlLoopDefinition, model: GeneralModel, family: "P" | "PI" | "PD" | "PID", design: PidPiDesign,
  task: EngineerRequirements): string | "meets" | "unpredicted" {
  const range = gainRangeShortfall(loop, design);
  if (range) {
    // PD's steady error is the offset divided by Kp, so the allowance sets its gain; integral action removes the offset.
    return family === "PD" || family === "P" ? `${range}, to keep its steady-state error within ${formatEngineeringNumber(Number((task.maximumError - task.settlingBand).toPrecision(2)))} `
      + `${displayEngineeringUnit(loop.signals.processVariable.unit)}. Choose a structure with integral action or allow a larger error.`
      : `${range}. Allow a longer settling time or a larger error.`;
  }
  const prediction = engineerPrediction(model, loop, design.candidate, task);
  const deadline = responseDeadlineSeconds(task.settlingTimeSeconds, loop.controller.taskPeriodMs);
  const remaining = prediction?.timeResponse.filter(point => point.timeSeconds >= deadline - 1e-9).map(point => point.candidatePv) ?? [];
  if (!remaining.length) return "unpredicted";
  const target = (model.operatingPoint?.outputs[0] ?? 0) + task.referenceChange;
  const final = remaining[remaining.length - 1];
  const error = Math.max(...remaining.map(value => Math.abs(value - target)));
  const movement = Math.max(...remaining.map(value => Math.abs(value - final)));
  const unit = displayEngineeringUnit(loop.signals.processVariable.unit);
  const value = (number: number) => `${formatEngineeringNumber(Number(number.toPrecision(2)))} ${unit}`;
  const after = `after ${formatEngineeringNumber(deadline)} s`, slow = "Allow a longer settling time or a larger error.";
  if ((family === "PD" || family === "P") && error > task.maximumError && movement <= task.settlingBand) {
    return `settles with a steady-state error of ${value(Math.abs(final - target))}, where ${value(task.maximumError)} is allowed. `
      + "Choose a structure with integral action or allow a larger error.";
  }
  if (error > task.maximumError) return `is up to ${value(error)} from the target ${after}, where ${value(task.maximumError)} is allowed. ${slow}`;
  if (movement > task.settlingBand) return `still moves ${value(movement)} ${after}, more than the ±${value(task.settlingBand)} settling band. ${slow}`;
  return task.constantInputDisturbance ? loadShortfall(loop, model, family, design, task) : "meets";
}

/** The load trial on the model: the setpoint stays, the declared input disturbance acts from the start. */
function loadShortfall(loop: ControlLoopDefinition, model: GeneralModel, family: "P" | "PI" | "PD" | "PID", design: PidPiDesign,
  task: EngineerRequirements): string | "meets" | "unpredicted" {
  const load = task.constantInputDisturbance!;
  const prediction = engineerPrediction(model, loop, design.candidate, { ...task, referenceChange: 0 }, undefined, load);
  const deadline = responseDeadlineSeconds(task.settlingTimeSeconds, loop.controller.taskPeriodMs);
  const remaining = prediction?.timeResponse.filter(point => point.timeSeconds >= deadline - 1e-9).map(point => point.candidatePv) ?? [];
  if (!remaining.length) return "unpredicted";
  const target = model.operatingPoint?.outputs[0] ?? 0, final = remaining[remaining.length - 1];
  const error = Math.max(...remaining.map(value => Math.abs(value - target)));
  const movement = Math.max(...remaining.map(value => Math.abs(value - final)));
  const unit = displayEngineeringUnit(loop.signals.processVariable.unit);
  const value = (number: number) => `${formatEngineeringNumber(Number(number.toPrecision(2)))} ${unit}`;
  const under = `under the ${formatEngineeringNumber(load)} ${displayEngineeringUnit(loop.signals.manipulatedVariable.unit)} input disturbance`;
  const after = `after ${formatEngineeringNumber(deadline)} s`, slow = "Allow a longer settling time or a larger error.";
  if ((family === "PD" || family === "P") && error > task.maximumError && movement <= task.settlingBand) {
    return `settles ${under} with a steady-state error of ${value(Math.abs(final - target))}, where ${value(task.maximumError)} is allowed. `
      + "Choose a structure with integral action or allow a larger error.";
  }
  if (error > task.maximumError) return `is ${under} up to ${value(error)} from the target ${after}, where ${value(task.maximumError)} is allowed. ${slow}`;
  if (movement > task.settlingBand) return `still moves ${under} ${value(movement)} ${after}, more than the ±${value(task.settlingBand)} settling band. ${slow}`;
  return "meets";
}

/** A design whose gain leaves the range the loop declares for it cannot be applied; say which gain and by how much. */
function gainRangeShortfall(loop: ControlLoopDefinition, design: PidPiDesign): string | undefined {
  const pid = decodePidLaw(loop.controller.law, loop.parameterBinding);
  const seed = loop.controller.project?.seed, own = seed?.kind === "parallel-pid" || seed?.kind === "trajectory-feedforward-pid" ? seed : undefined;
  const gains: [string, string | undefined, number | undefined][] = [
    ["Kp", pid?.proportionalGainSymbol ?? own?.proportionalSymbol, design.candidate.proportionalGain],
    ["Ki", pid?.integralGainSymbol ?? own?.integralSymbol, design.candidate.integralGain],
    ["Kd", pid?.derivativeGainSymbol ?? own?.derivativeSymbol, design.candidate.derivativeGain],
  ];
  for (const [label, symbol, value] of gains) {
    const parameter = loop.parameterBinding?.parameters.find(candidate => candidate.symbol === symbol);
    if (!parameter || value === undefined || !Number.isFinite(value)) continue;
    const unit = displayEngineeringUnit(parameter.unit), shown = (number: number) => formatEngineeringNumber(Number(number.toPrecision(4)));
    if (value > parameter.maximum) return `needs ${label} = ${shown(value)} ${unit}, above its declared maximum of ${shown(parameter.maximum)} ${unit}`;
    if (value < parameter.minimum) return `needs ${label} = ${shown(value)} ${unit}, below its declared minimum of ${shown(parameter.minimum)} ${unit}`;
  }
  return undefined;
}

/**
 * P on a delayed integrating model (Specification 35 §9.2): SIMC's proportional gain Kp = 1/(K′(λ + L)), λ = L, 2L or 4L
 * from the declared response outward, kept when its prediction, the declared input disturbance included, meets the task.
 */
export function designIntegratingPToRequirements(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  rateGain: number, delaySeconds: number): PidPiDesign {
  const multiples: Record<Response, number> = { fast: 1, balanced: 2, robust: 4 };
  const shortfalls: Partial<Record<Response, string>> = {};
  for (const [index, response] of NEAREST[task.tuningObjective ?? "balanced"].entries()) {
    const lambda = multiples[response] * delaySeconds;
    const candidate = designTypedProportionalCandidate(model, 1 / (rateGain * (lambda + delaySeconds)));
    const outcome = predictedShortfall(loop, model, "P", candidate, task);
    if (outcome === "meets" || outcome === "unpredicted" && index === 0) return candidate;
    if (outcome !== "unpredicted") shortfalls[response] = outcome;
  }
  const reason = shortfalls.fast ? `even its fastest response ${shortfalls.fast}` : `its predicted response ${shortfalls.balanced ?? shortfalls.robust}`;
  throw new MainWorkflowError(`P cannot meet the requirements on the model: ${reason}`, { cause: "task-infeasible", nextAction: "revise-requirements" });
}

/** K′e^(−Ls)/s with L > 0: the rate gain and delay of a delayed integrating model, or nothing for any other model. */
export function delayedIntegrator(model: GeneralModel): { rateGain: number; delaySeconds: number } | undefined {
  let transfer: ReturnType<typeof linearTransferModel>;
  try { transfer = linearTransferModel(model); } catch { return undefined; }
  const [leading, constant] = transfer.denominator;
  if (transfer.denominator.length !== 2 || constant !== 0 || transfer.numerator.length !== 1 || !(transfer.delaySeconds > 0)) return undefined;
  const rateGain = transfer.numerator[0] / leading;
  return Number.isFinite(rateGain) && rateGain > 0 ? { rateGain, delaySeconds: transfer.delaySeconds } : undefined;
}
