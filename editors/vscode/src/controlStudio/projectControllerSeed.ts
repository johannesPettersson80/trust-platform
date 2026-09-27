import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { designPidForTask, poleMatchedPid } from "./pidTaskDesign";
import { designSimpleProportionalController } from "./simpleControllerDesign";

export interface ProjectControllerSeed {
  kind: "parallel-pid";
  family: "P" | "PI" | "PD" | "PID";
  proportionalSymbol: string;
  integralSymbol: string;
  derivativeSymbol: string;
}
export function projectControllerSeed(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  seed: ProjectControllerSeed | undefined, family = seed?.family): Record<string, number> {
  if (!seed) return {};
  if (seed.kind !== "parallel-pid" || !family || !["P", "PI", "PD", "PID"].includes(family)) throw new Error("Declare a supported typed controller seed.");
  const candidate = family === "P" ? (() => {
    const parameter = loop.parameterBinding?.parameters.find(parameter => parameter.symbol === seed.proportionalSymbol);
    if (!parameter) throw new Error("The proportional seed parameter is not bound.");
    try { return { ...designSimpleProportionalController(model, { ...task, constantInputDisturbance: task.constantInputDisturbance ?? 0,
      maximumSteadyError: task.maximumError - task.settlingBand, proportionalGainBounds: { minimum: parameter.minimum, maximum: parameter.maximum } }).candidate,
      integralGain: 0, derivativeGain: 0 }; }
    catch { return poleMatchedPid(model, "P", task, loop.controller.taskPeriodMs).candidate; }
  })() : designPidForTask(model, family as "PI" | "PD" | "PID", task, loop.controller.taskPeriodMs).candidate;
  return { [seed.proportionalSymbol]: candidate.proportionalGain, [seed.integralSymbol]: candidate.integralGain, [seed.derivativeSymbol]: candidate.derivativeGain };
}
