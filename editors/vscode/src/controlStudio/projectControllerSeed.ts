import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { poleMatchedPid } from "./pidTaskDesign";
import { designPidToRequirements } from "./automaticPidResponse";
import { designSimpleProportionalController } from "./simpleControllerDesign";
import { designDecentralizedPi } from "./decentralizedPidDesign";
import { designCascade, type CascadePidSeed } from "./cascadePidDesign";
import { feedforwardDesign, type FeedforwardPidSeed } from "./feedforwardPidDesign";
import { designSmithPredictor, type SmithPidSeed } from "./smithPredictorDesign";
import { decoupledGroupDesign, type DecoupledPidSeed } from "./decoupledGroupDesign";
import { linearTransferModel } from "./linearTransferModel";

export type ProjectControllerSeed = ParallelPidSeed | DecentralizedPidSeed | CascadePidSeed | FeedforwardPidSeed | SmithPidSeed | DecoupledPidSeed
  | TrajectoryFeedforwardSeed;
export interface ParallelPidSeed {
  kind: "parallel-pid";
  family: "P" | "PI" | "PD" | "PID";
  proportionalSymbol: string;
  /** Absent when the program's law has no integral gain. */
  integralSymbol?: string;
  /** Absent when the program's law has no derivative gain. */
  derivativeSymbol?: string;
}
/** A PID whose program adds `K_v·ṙ + K_a·r̈` of its planned trajectory to the output (Specification 35 §17.16). */
export interface TrajectoryFeedforwardSeed {
  kind: "trajectory-feedforward-pid"; family: "P" | "PI" | "PD" | "PID";
  proportionalSymbol: string; integralSymbol: string; derivativeSymbol: string;
  rateSymbol: string; accelerationSymbol: string;
}
/**
 * The PID gains a `parallel-pid` or `trajectory-feedforward-pid` seed's parameters stand for, so a program's own law can be
 * analysed on the model; a trajectory's feedforward acts outside the loop.
 */
export function parallelSeedGains(seed: ProjectControllerSeed | undefined, parameters: Readonly<Record<string, number>>):
  { proportionalGain: number; integralGain: number; derivativeGain: number } | undefined {
  if (seed?.kind !== "parallel-pid" && seed?.kind !== "trajectory-feedforward-pid" || !Number.isFinite(parameters[seed.proportionalSymbol])) return undefined;
  return { proportionalGain: parameters[seed.proportionalSymbol], integralGain: seed.integralSymbol ? parameters[seed.integralSymbol] ?? 0 : 0,
    derivativeGain: seed.derivativeSymbol ? parameters[seed.derivativeSymbol] ?? 0 : 0 };
}

/** A PID group's members, each designed on its own path of the group model (Specification 35 §17.9). */
export interface DecentralizedPidSeed { kind: "decentralized-pid"; family: "PI" }

export function projectControllerSeed(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  seed: ProjectControllerSeed | undefined, family = seed?.family): Record<string, number> {
  if (!seed) return {};
  if (seed.kind === "decentralized-pid") return decentralizedSeed(loop, model, task);
  if (seed.kind === "cascade-pid") return designCascade(loop, model, task, seed);
  if (seed.kind === "feedforward-pid") return feedforwardDesign(loop, model, task, seed).values;
  if (seed.kind === "smith-pid") return designSmithPredictor(loop, model, task, seed);
  if (seed.kind === "decoupled-pid") return decoupledGroupDesign(loop, model, task, seed).values;
  if (seed.kind === "trajectory-feedforward-pid" && family && ["P", "PI", "PD", "PID"].includes(family)) {
    // The feedforward is the model's inverse along the trajectory; the PID is designed as for Control.PID (§17.16).
    const feedforward = trajectoryFeedforward(model);
    return { ...familyGains(loop, model, task, seed, family as ParallelPidSeed["family"]),
      [seed.rateSymbol]: feedforward.rateGain, [seed.accelerationSymbol]: feedforward.accelerationGain };
  }
  if (seed.kind !== "parallel-pid" || !family || !["P", "PI", "PD", "PID"].includes(family)) throw new Error("Declare a supported typed controller seed.");
  return familyGains(loop, model, task, seed, family as ParallelPidSeed["family"]);
}

/** The P, PI, PD or PID gains a program's law is given, designed to the requirements as Automatic designs Control.PID. */
function familyGains(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  seed: { proportionalSymbol: string; integralSymbol?: string; derivativeSymbol?: string }, family: ParallelPidSeed["family"]): Record<string, number> {
  if (family.includes("I") && !seed.integralSymbol) throw new Error(`A ${family} design needs an integral gain; the program's law has no integral gain.`);
  if (family.includes("D") && !seed.derivativeSymbol) throw new Error(`A ${family} design needs a derivative gain; the program's law has no derivative gain.`);
  const candidate = family === "P" ? (() => {
    const parameter = loop.parameterBinding?.parameters.find(parameter => parameter.symbol === seed.proportionalSymbol);
    if (!parameter) throw new Error("The proportional seed parameter is not bound.");
    try { return { ...designSimpleProportionalController(model, { ...task, constantInputDisturbance: task.constantInputDisturbance ?? 0,
      maximumSteadyError: task.maximumError - task.settlingBand, proportionalGainBounds: { minimum: parameter.minimum, maximum: parameter.maximum } }).candidate,
      integralGain: 0, derivativeGain: 0 }; }
    catch { return poleMatchedPid(model, "P", task, loop.controller.taskPeriodMs, loop.controller.action).candidate; }
  // A program's PI/PD/PID is designed to the requirements as Automatic designs Control.PID (Specification 35 §9.2).
  })() : designPidToRequirements(loop, model, family as "PI" | "PD" | "PID", task).candidate;
  return { [seed.proportionalSymbol]: candidate.proportionalGain, ...(seed.integralSymbol ? { [seed.integralSymbol]: candidate.integralGain } : {}),
    ...(seed.derivativeSymbol ? { [seed.derivativeSymbol]: candidate.derivativeGain } : {}) };
}

/** For `n/(a·s² + b·s)`, `K_v = b/n` and `K_a = a/n`: the output with which the model follows the trajectory exactly. */
export function trajectoryFeedforward(model: GeneralModel): { rateGain: number; accelerationGain: number } {
  const refused = "A trajectory feedforward needs a model n/(a·s² + b·s), an integrator with or without a lag; this model has no trajectory feedforward rule.";
  let transfer: GeneralModel;
  try { transfer = linearTransferModel(model); } catch { throw new Error(refused); }
  if (transfer.kind !== "transfer-function") throw new Error(refused);
  const numerator = transfer.numerator.slice(transfer.numerator.findIndex(value => value !== 0));
  const denominator = transfer.denominator.slice(transfer.denominator.findIndex(value => value !== 0));
  const [a, b] = denominator.length === 3 ? [denominator[0], denominator[1]] : denominator.length === 2 ? [0, denominator[0]] : [NaN, NaN];
  if (numerator.length !== 1 || numerator[0] === 0 || denominator[denominator.length - 1] !== 0 || !(b !== 0) || !Number.isFinite(a)) throw new Error(refused);
  return { rateGain: b / numerator[0], accelerationGain: a / numerator[0] };
}

/** The decentralized PI design of a PID group's members on the group model (Specification 35 §17.9). */
export function decentralizedGroupDesign(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements) {
  const members = loop.controller.group?.members;
  if (!members?.length) throw new Error("A decentralized PI seed needs the loop's PID group members.");
  return designDecentralizedPi(model, members.map(member => ({ inputId: member.inputChannelId, outputId: member.outputChannelId })),
    task.tuningObjective ?? "balanced", loop.controller.action);
}

function decentralizedSeed(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements): Record<string, number> {
  const members = loop.controller.group!.members;
  const design = decentralizedGroupDesign(loop, model, task);
  return Object.fromEntries(members.flatMap((member, index) => {
    const field = (name: string) => {
      const symbol = member.parameterSymbols.find(candidate => candidate.endsWith(`.${name}`));
      if (!symbol) throw new Error(`${member.instance} binds no ${name}.`);
      return symbol;
    };
    const designed = design.members[index];
    return [[field("ProportionalGain"), designed.proportionalGain], [field("IntegralGain"), designed.integralGain], [field("DerivativeGain"), 0]];
  }));
}
