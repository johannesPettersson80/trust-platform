import { engineerPrediction } from "./engineerPrediction";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { FeedforwardDesign } from "./feedforwardPidDesign";
import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import { firstOrderPathModel } from "./pathReduction";
import { parallelSeedGains } from "./projectControllerSeed";

/**
 * The linear loop whose margins Tune shows for a project candidate (Specification 35 §17.10): a program's own P/PI/PD/PID
 * on the selected model, or a feedforward's PI on the output's path, since the feedforward acts outside the loop (§17.12).
 */
export function projectMarginEvidence(model: GeneralModel | undefined, loop: ControlLoopDefinition,
  design: { candidate: unknown; feedforward?: FeedforwardDesign }, task: EngineerRequirements | undefined) {
  const seed = loop.controller.project?.seed, parameters = (design.candidate as { parameters?: Readonly<Record<string, number>> }).parameters ?? {};
  const seeded = parallelSeedGains(seed, parameters);
  if (seeded) return engineerPrediction(model, loop, seeded, task);
  if (seed?.kind !== "feedforward-pid" || !design.feedforward || !model) return undefined;
  const output = loop.signals.manipulatedVariable, measurement = loop.signals.processVariable;
  const input = model.inputs.findIndex(channel => channel.id === output.symbol);
  const path = firstOrderPathModel(model, design.feedforward.outputPath, output, measurement,
    model.operatingPoint?.inputs[input] ?? 0, model.operatingPoint?.outputs[0] ?? 0);
  return engineerPrediction(path, loop, { proportionalGain: parameters[seed.proportionalSymbol], integralGain: parameters[seed.integralSymbol],
    derivativeGain: 0 }, task);
}
