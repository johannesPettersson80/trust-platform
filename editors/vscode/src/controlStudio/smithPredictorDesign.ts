import type { GeneralModel } from "./generalModel";
import { designPidToRequirements } from "./automaticPidResponse";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { ControlLoopDefinition } from "./loopDefinition";
import { firstOrderPathModel, openLoopStepResponse, twoPointPath } from "./pathReduction";

/** A `Control.PID` fed the measurement plus a Smith predictor's correction, with the program's model variables (Specification 35 §17.13). */
export interface SmithPidSeed {
  kind: "smith-pid"; family: "PI";
  proportionalSymbol: string; integralSymbol: string; derivativeSymbol: string;
  gainSymbol: string; timeConstantSymbol: string; delaySymbol: string;
}

/** The model's first-order-plus-delay reduction as the predictor's model, and the PI designed for the process without its delay. */
export function designSmithPredictor(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements, seed: SmithPidSeed): Record<string, number> {
  if (model.inputs.length !== 1 || model.outputs.length !== 1) throw new Error("A Smith predictor is designed on a model from the controller output to the measurement.");
  const response = openLoopStepResponse(model, 0, "Smith predictor");
  const path = twoPointPath(response.times, response.outputs[0], "Smith predictor");
  // With a correct model the loop responds as the delay-free loop, delayed by θ: that loop must settle θ sooner.
  const settlingTimeSeconds = task.settlingTimeSeconds - path.delaySeconds;
  if (!(settlingTimeSeconds > 0)) {
    throw new Error(`The settling time must be longer than the model's delay of ${Number(path.delaySeconds.toPrecision(3))} s: no controller acts sooner.`);
  }
  const delayFree = firstOrderPathModel(model, { ...path, delaySeconds: 0 }, loop.signals.manipulatedVariable, loop.signals.processVariable,
    model.operatingPoint?.inputs[0] ?? 0, model.operatingPoint?.outputs[0] ?? 0);
  const pi = designPidToRequirements(loop, delayFree, "PI", { ...task, settlingTimeSeconds,
    durationSeconds: Math.max(task.durationSeconds - path.delaySeconds, 2 * settlingTimeSeconds) }).candidate;
  return { [seed.proportionalSymbol]: pi.proportionalGain, [seed.integralSymbol]: pi.integralGain, [seed.derivativeSymbol]: 0,
    [seed.gainSymbol]: path.gain, [seed.timeConstantSymbol]: path.timeConstantSeconds, [seed.delaySymbol]: path.delaySeconds };
}
