import { MainWorkflowError } from "./mainWorkflowFailure";
import { poleMatchedPid } from "./pidTaskDesign";
import { delayedIntegrator, designIntegratingPToRequirements, designPidToRequirements } from "./automaticPidResponse";
import { linearTransferModel } from "./linearTransferModel";
import { decodePidLaw, materializePidGainRecord } from "./controllerLaw";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { proportionalParameter } from "./engineerVerificationPlan";
import { designSimpleProportionalController, type SimpleProportionalDesign } from "./simpleControllerDesign";
import { designTypedProportionalCandidate, type PidPiDesign } from "./pidPiDesign";
import { controllerAdapter } from "./controllerAdapters";

export type AutomaticControllerFamily = "P" | "PI" | "PD" | "PID";
export type AutomaticControllerAttempt = {
  structure: AutomaticControllerFamily;
  status: "Feasible" | "Rejected";
  reason: string;
};

export class AutomaticControllerSelectionError extends Error {
  readonly attempts: readonly AutomaticControllerAttempt[];
  constructor(attempts: readonly AutomaticControllerAttempt[]) {
    super(`No compatible automatic controller candidate: ${attempts.map(attempt => `${attempt.structure}: ${attempt.reason}`).join("; ")}`);
    this.name = "AutomaticControllerSelectionError";
    this.attempts = structuredClone(attempts);
  }
}

export interface AutomaticControllerRecommendation {
  family: AutomaticControllerFamily;
  design: SimpleProportionalDesign | PidPiDesign;
  values: Record<string, number>;
  attempts: readonly AutomaticControllerAttempt[];
  reason: string;
}

/**
 * Chooses the least-complexity controller that the declared model, ST law and
 * requirements establish as a candidate. Every returned choice still requires
 * actual-ST verification; this is deliberately not a sampled gain sweep.
 */
export function recommendAutomaticController(
  loop: ControlLoopDefinition,
  model: GeneralModel,
  task: EngineerRequirements,
  applicationMode: "auto" | "manual",
): AutomaticControllerRecommendation {
  const topology = controllerTopology(loop, model);
  if (topology) {
    const attempts = (['P', 'PI', 'PD', 'PID'] as const).map(structure => ({
      structure, status: "Rejected" as const, reason: topology,
    }));
    throw new AutomaticControllerSelectionError(attempts);
  }
  const pid = decodePidLaw(loop.controller.law, loop.parameterBinding);
  const proportionalBinding = pid
    ? loop.parameterBinding?.parameters.find(parameter => parameter.symbol === pid.proportionalGainSymbol)
    : proportionalParameter(loop);
  if (!proportionalBinding) throw new Error("The declared automatic controller comparison has no compatible proportional gain binding.");
  const requirements = {
    referenceChange: task.referenceChange,
    constantInputDisturbance: task.constantInputDisturbance ?? 0,
    maximumSteadyError: task.maximumError - task.settlingBand,
    settlingBand: task.settlingBand,
    settlingTimeSeconds: task.settlingTimeSeconds,
    proportionalGainBounds: { minimum: proportionalBinding.minimum, maximum: proportionalBinding.maximum },
  };
  const attempts: AutomaticControllerAttempt[] = [];
  let p: AutomaticControllerRecommendation | undefined;
  try {
    if (pid && applicationMode === "auto") controllerAdapter(loop).checkInitialization();
    const proportional = (() => {
      // A delayed integrator keeps P eligible with SIMC's gain (Specification 35 §9.2).
      const integrator = pid ? delayedIntegrator(model) : undefined;
      if (integrator) return designIntegratingPToRequirements(loop, model, task, integrator.rateGain, integrator.delaySeconds);
      try { return designSimpleProportionalController(model, requirements); }
      catch { return poleMatchedPid(model, "P", task, loop.controller.taskPeriodMs, loop.controller.action); }
    })();
    assertInitialOutputWithinLimits(loop, proportional.candidate.proportionalGain, task, loop.controller.pidInitialization ? model.operatingPoint?.inputs[0] ?? 0 : 0);
    p = pid
      ? (() => {
        const design = designTypedProportionalCandidate(model, proportional.candidate.proportionalGain);
        return { family: "P" as const, design, values: materializePidGainRecord(loop, design.candidate, {}), attempts,
          reason: "P is the simplest model-screened candidate; press Verify to execute the actual ST." };
      })()
      : { family: "P", design: proportional, values: { [proportionalBinding.symbol]: proportional.candidate.proportionalGain }, attempts,
        reason: "P is the simplest model-screened candidate; press Verify to execute the actual ST." };
    attempts.push({ structure: "P", status: "Feasible", reason: "The declared model prediction satisfies the task; actual-ST Verify remains required." });
  } catch (error) {
    attempts.push({ structure: "P", status: "Rejected", reason: explanation(error) });
  }
  if (!pid) {
    if (p) return p;
    throw new MainWorkflowError(attempts[0].reason, { cause: "task-infeasible", nextAction: "open-requirements" });
  }

  const pi = typedCandidate("PI", () => designPidToRequirements(loop, model, "PI", task), loop, model, task, applicationMode);
  attempts.push(pi.attempt);
  const pd = typedCandidate("PD", () => designPidToRequirements(loop, model, "PD", task), loop, model, task, applicationMode);
  attempts.push(pd.attempt);
  const pidCandidate = typedCandidate("PID", () => designPidToRequirements(loop, model, "PID", task), loop, model, task, applicationMode);
  attempts.push(pidCandidate.attempt);
  if (p) return { ...p, attempts,
    reason: "Automatic comparison retained P, PI, PD and PID attempts. P is the simplest model-screened candidate; the selected P requires actual-ST Verify." };
  if (pi.design && pi.values) {
    return { family: "PI", design: pi.design, values: pi.values, attempts,
      reason: "PI is the simplest compatible candidate because the P design was rejected for the declared model, bounds or requirements; press Verify to execute the actual ST." };
  }
  if (pd.design && pd.values) {
    return { family: "PD", design: pd.design, values: pd.values, attempts,
      reason: "PD is a compatible candidate after the P and PI attempts were rejected; actual-ST Verify must establish whether it meets the task." };
  }
  if (pidCandidate.design && pidCandidate.values) {
    return { family: "PID", design: pidCandidate.design, values: pidCandidate.values, attempts,
      reason: "PID is the simplest compatible candidate after P and PI were rejected; press Verify to execute the actual ST." };
  }
  const branch = unsupportedPlantBranch(model);
  throw new AutomaticControllerSelectionError(branch
    ? attempts.map(attempt => ({ ...attempt, reason: `${attempt.reason} ${branch}` }))
    : attempts);
}

/**
 * The shared scalar tuner must fail closed when the loop declares a structure
 * owned by another adapter.  The caller routes these branches before entering
 * this function; keeping the message here also makes direct use explain the
 * compatible alternatives instead of reporting a generic SISO failure.
 */
function controllerTopology(loop: ControlLoopDefinition, model: GeneralModel): string | undefined {
  if (loop.controller.onOff || loop.channels?.inputs.some(input => input.actuator.kind === "on-off")) {
    return "The ON/OFF adapter owns this actuator: use its hysteresis and dwell design instead of continuous P/PI/PD/PID tuning.";
  }
  if (model.inputs.length !== 1 || model.outputs.length !== 1) {
    return "This is a multivariable loop. Use the declared state-feedback/LQR/LQG adapter or a compatible project-owned decoupled/predictive adapter.";
  }
  return undefined;
}

function unsupportedPlantBranch(model: GeneralModel): string | undefined {
  if (model.kind === "transfer-function" && model.numerator.length > 1) {
    return "The selected model has explicit zero dynamics (possibly inverse response); this scalar adapter has no qualified zero-cancellation or inverse-response design. Use a declared state-feedback or project-owned adapter.";
  }
  if (model.kind === "transfer-function" && model.denominator.length > 3) {
    return "The selected model has oscillatory or higher-order dynamics outside the shared coefficient-matching domain. Use a declared state-feedback or project-owned adapter, or revise the model structure.";
  }
  return undefined;
}

function typedCandidate(
  family: "PI" | "PD" | "PID",
  create: () => PidPiDesign,
  loop: ControlLoopDefinition,
  model: GeneralModel,
  task: EngineerRequirements,
  applicationMode: "auto" | "manual",
): { attempt: AutomaticControllerAttempt; design?: PidPiDesign; values?: Record<string, number> } {
  try {
    if (applicationMode === "auto") controllerAdapter(loop).checkInitialization();
    const design = create();
    if (family === "PD") {
      const transfer = linearTransferModel(model);
      const a0 = transfer.denominator[transfer.denominator.length - 1], b = transfer.numerator[transfer.numerator.length - 1];
      const denominator = a0 + b * design.candidate.proportionalGain;
      const steadyError = Math.max(Math.abs(a0 * task.referenceChange + b * (model.operatingPoint?.inputs[0] ?? 0)),
        Math.abs(b * ((model.operatingPoint?.inputs[0] ?? 0) - (task.constantInputDisturbance ?? 0)))) / denominator;
      if (denominator <= 0 || !Number.isFinite(steadyError) || steadyError > task.maximumError - task.settlingBand) {
        throw new Error(`PD predicted steady error ${steadyError} exceeds the declared design allowance.`);
      }
    }
    assertInitialOutputWithinLimits(loop, design.candidate.proportionalGain, task, loop.controller.pidInitialization ? model.operatingPoint?.inputs[0] ?? 0 : 0);
    return { design, values: materializePidGainRecord(loop, design.candidate, {}), attempt: {
      structure: family, status: "Feasible", reason: "Compatible typed candidate retained for actual-ST Verify.",
    } };
  } catch (error) {
    return { attempt: { structure: family, status: "Rejected", reason: explanation(error) } };
  }
}

/** The declared P term is the first automatic output from zero model deviations; no hidden bias is added. */
function assertInitialOutputWithinLimits(
  loop: ControlLoopDefinition,
  proportionalGain: number,
  task: EngineerRequirements,
  operatingBias = 0,
): void {
  const output = operatingBias + proportionalGain * task.referenceChange;
  const { outputMinimum: minimum, outputMaximum: maximum } = loop.limits;
  if (!Number.isFinite(output) || output < minimum || output > maximum) {
    throw new Error(`Automatic controller initial output ${output} is outside the declared output limit ${minimum}…${maximum}.`);
  }
}

function explanation(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
