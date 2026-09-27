import { designPidForTask } from "./pidTaskDesign";
import { assertPidGroupAdapter } from "./pidControllerGroup";
import { libraryPidInitialization } from "./libraryPidInitialization";
import { decodeProjectController, projectControllerFamilies } from "./projectController";
import type { ControllerInitialization } from "./controllerInitialization";
import { decodeOnOffController } from "./onOffController";
import { materializeBoundParameters } from "./parameterBinding";
import { decodeControllerInitialization, declaredTransferBinding, declaredCommissioningProfile, declaredInitializationPlan } from "./controllerInitialization";
import { MainWorkflowError } from "./mainWorkflowFailure";
import { decodeStateControllerBinding } from "./stateControllerDesign";
import type { ControllerFamily } from "./controllerFamilies";
export type { ControllerFamily } from "./controllerFamilies";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { proportionalParameter } from "./engineerVerificationPlan";
import { decodePidLaw, materializePidGainRecord } from "./controllerLaw";
import { designTypedDerivativeCandidate, designTypedPiCandidate, type PidPiDesign } from "./pidPiDesign";
import { pidInitializationAdapter } from "./pidInitialization";
import { pidInitializationPlan } from "./pidInitializationPlan";
import { proportionalTransferBinding, proportionalTracking, type TransferBinding } from "./proportionalTransfer";
import { pidTransferBinding, parallelPidTracking } from "./pidTransfer";
import type { TransferTracking } from "./runtimeTransfer";
import { proportionalInitializationPlan } from "./proportionalTransfer";
import { PidCommissioning } from "./pidCommissioning";
import { ProportionalCommissioning, type CommissioningResult } from "./proportionalCommissioning";
import type { ParameterApplicationClient, ParameterReview } from "./parameterApplication";

export interface ControllerGains { parameters?: Readonly<Record<string, number>>; proportionalGain?: number; integralGain?: number; derivativeGain?: number; backCalculationGain?: number }
export interface ControllerAdapter {
  transferBinding(): TransferBinding;
  recoveryTracking(model: GeneralModel): TransferTracking;
  readonly families: readonly ControllerFamily[];
  materialize(gains: ControllerGains, observed: Readonly<Record<string, number>>): Record<string, number>;
  checkInitialization(): void;
  assertRecoveryNeeded(report: CommissioningResult, model?: GeneralModel): void;
  recover(model: GeneralModel, task: EngineerRequirements, client: ParameterApplicationClient, review: ParameterReview, report: CommissioningResult): Promise<PidCommissioning | ProportionalCommissioning>;
  initialization(model: GeneralModel, task: EngineerRequirements, values: Record<string, number>): ReturnType<typeof proportionalInitializationPlan>;
  commission(model: GeneralModel, task: EngineerRequirements, client: ParameterApplicationClient, review: ParameterReview): Promise<PidCommissioning | ProportionalCommissioning>;
}

/** One owner for actual ST conventions throughout Tune, Verify and Apply. */
export function controllerAdapter(loop: ControlLoopDefinition): ControllerAdapter {
  const project = decodeProjectController(loop.controller.project, loop.parameterBinding);
  assertPidGroupAdapter(loop, project);
  if (project) return mappedControllerAdapter(loop, projectControllerFamilies(project), loop.parameterBinding!.parameters.map(parameter => parameter.symbol), project.initialization, project.family);
  const switching = decodeOnOffController(loop.controller.onOff, loop.parameterBinding);
  if (switching) return mappedControllerAdapter(loop, ["ON/OFF"],
    [switching.hysteresisSymbol, switching.minimumOnTimeSymbol, switching.minimumOffTimeSymbol], switching.initialization, "ON/OFF");
  const state = decodeStateControllerBinding(loop.controller.stateFeedback, loop.parameterBinding);
  if (state) return mappedControllerAdapter(loop,
    state.family === "LQG" ? ["LQG"] : state.gainSymbols.length === 1 ? ["state-feedback", "LQR"] : ["LQR"],
    [...state.gainSymbols.flat(), ...state.referenceGainSymbols.flat(), ...(state.observerGainSymbols?.flat() ?? [])], state.initialization, state.family);
  if (decodePidLaw(loop.controller.law, loop.parameterBinding)) {
    if (loop.channels && (loop.channels.inputs.length > 1 || loop.channels.outputs.length > 1)) {
      throw new MainWorkflowError("The shared scalar PID adapter cannot own a multivariable loop. Declare state-feedback/LQR/LQG or a project-owned decoupled/predictive adapter for these channels.", { cause: "controller-capability", nextAction: "choose-controller" });
    }
    if (loop.controller.pidInitialization?.kind === "control-pid-v1") {
      const mapped = mappedControllerAdapter(loop, ["P", "PI", "PD", "PID"], [], libraryPidInitialization(loop), "Control.PID");
      return { ...mapped, materialize: (gains, observed) => {
        if (typeof gains.proportionalGain !== "number") throw new Error("A proportional gain is required by Control.PID.");
        return materializePidGainRecord(loop, { ...gains, proportionalGain: gains.proportionalGain,
          integralGain: gains.integralGain ?? 0, derivativeGain: gains.derivativeGain ?? 0 }, observed);
      } };
    }
    return {
    transferBinding: () => pidTransferBinding(loop),
    recoveryTracking: () => parallelPidTracking(loop, pidTransferBinding(loop), false),
    families: ["P", "PI", "PD", "PID"],
    materialize: (gains, observed) => {
      if (typeof gains.proportionalGain !== "number") throw new Error("A proportional gain is required by the declared PID record.");
      return materializePidGainRecord(loop, { ...gains, proportionalGain: gains.proportionalGain,
        integralGain: gains.integralGain ?? 0, derivativeGain: gains.derivativeGain ?? 0 }, observed);
    },
    checkInitialization: () => { pidInitializationAdapter(loop); },
    assertRecoveryNeeded: report => PidCommissioning.assertRecoveryNeeded(loop, report),
    recover: (model, task, client, review, report) => PidCommissioning.reviewRecovery(loop, model, task, client, review, report),
    initialization: (model, task, values) => pidInitializationPlan(loop, model, task, values),
    commission: (model, task, client, review) => PidCommissioning.review(loop, model, task, client, review),
    };
  }
  const parameter = proportionalParameter(loop);
  return {
    transferBinding: () => proportionalTransferBinding(loop),
    recoveryTracking: () => proportionalTracking(loop, proportionalTransferBinding(loop), false),
    families: ["P"],
    materialize: (gains, observed) => {
      if (typeof gains.proportionalGain !== "number") throw new Error("The proportional ST law needs a proportional gain.");
      if (gains.integralGain || gains.derivativeGain) throw new Error("The bound proportional ST law cannot realize integral or derivative gains.");
      const value = parameter.iec_type === "REAL" ? Math.fround(gains.proportionalGain) : gains.proportionalGain;
      if (!Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum) throw new Error("Realized proportional gain is outside its declared bounds.");
      return { ...observed, [parameter.symbol]: value };
    },
    checkInitialization: () => { proportionalParameter(loop); },
    assertRecoveryNeeded: report => ProportionalCommissioning.assertRecoveryNeeded(loop, report),
    recover: (model, task, client, review, report) => ProportionalCommissioning.reviewRecovery(loop, model, task, client, review, report),
    initialization: (model, task, values) => proportionalInitializationPlan(loop, model, task, values),
    commission: (model, task, client, review) => ProportionalCommissioning.review(loop, model, task, client, review),
  };
}

export function tuneTypedController(loop: ControlLoopDefinition, model: GeneralModel, family: "PI" | "PD" | "PID",
  applicationMode: "auto" | "manual", task?: EngineerRequirements,
  pinnedValues?: Readonly<Record<string, number>>): { design: PidPiDesign; values: Record<string, number> } {
  const adapter = controllerAdapter(loop);
  if (!adapter.families.includes(family)) throw new Error(`The bound ST adapter cannot realize ${family}.`);
  if (applicationMode === "auto") adapter.checkInitialization();
  const method = { method: task?.tuningMethod ?? "SIMC" as const, objective: task?.tuningObjective ?? "balanced" as const };
  const design = task ? designPidForTask(model, family, task, loop.controller.taskPeriodMs)
    : family === "PI" ? designTypedPiCandidate(model, method) : designTypedDerivativeCandidate(model, family, method);
  const law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  if (law?.antiWindup) {
    const integralTime = design.candidate.integralGain !== 0 ? Math.abs(design.candidate.proportionalGain / design.candidate.integralGain) : Infinity;
    const derivativeTime = design.candidate.proportionalGain !== 0 ? Math.abs(design.candidate.derivativeGain / design.candidate.proportionalGain) : 0;
    const trackingTime = Math.max(loop.controller.taskPeriodMs / 1000,
      derivativeTime > 0 ? Math.sqrt(integralTime * derivativeTime) : integralTime);
    design.candidate.backCalculationGain = Number.isFinite(trackingTime) ? 1 / trackingTime : 0;
  }
  const candidate = pinnedValues && Object.keys(pinnedValues).length
    ? applyPidPins(loop, family, design.candidate, pinnedValues)
    : design.candidate;
  const retained: PidPiDesign = candidate === design.candidate ? design : { ...design, candidate: { ...design.candidate, ...candidate } };
  return { design: retained, values: adapter.materialize(retained.candidate, {}) };
}

function applyPidPins(loop: ControlLoopDefinition, family: "PI" | "PD" | "PID",
  candidate: PidPiDesign["candidate"], pins: Readonly<Record<string, number>>): PidPiDesign["candidate"] {
  const law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  if (!law) throw new Error("Pinned PID settings require the declared parallel PID law.");
  type GainKey = "proportionalGain" | "integralGain" | "derivativeGain" | "backCalculationGain";
  const symbols: Record<string, GainKey> = {
    [law.proportionalGainSymbol]: "proportionalGain", [law.integralGainSymbol]: "integralGain", [law.derivativeGainSymbol]: "derivativeGain",
    ...(law.antiWindup ? { [law.antiWindup.trackingGainSymbol]: "backCalculationGain" as const } : {}),
  };
  const result = { ...candidate };
  for (const [symbol, value] of Object.entries(pins)) {
    const key = symbols[symbol];
    const parameter = loop.parameterBinding?.parameters.find(entry => entry.symbol === symbol);
    if (!key || !parameter || !Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum
      || parameter.iec_type === "REAL" && Math.fround(value) !== value) throw new Error(`Pinned setting ${symbol} does not match the bound PID parameter.`);
    result[key] = value;
  }
  if ((family === "PI" && result.derivativeGain !== 0) || (family === "PD" && result.integralGain !== 0)) {
    throw new Error("Pinned integral/derivative settings conflict with the selected controller family.");
  }
  return result;
}

/** All declared coefficient families share the same mapping and stateful lifecycle. */
function mappedControllerAdapter(loop: ControlLoopDefinition, families: readonly ControllerFamily[], symbols: readonly string[],
  initialization: ControllerInitialization | undefined, label: string): ControllerAdapter {
  const requiredInitialization = () => {
    const definition = decodeControllerInitialization(initialization);
    if (!definition) throw new MainWorkflowError("Declare controller initialization before Auto commissioning; verified coefficients can be staged in Manual.", { cause: "controller-capability", nextAction: "choose-application-mode" });
    try { declaredTransferBinding(loop, definition); }
    catch (error) { throw new MainWorkflowError(error instanceof Error ? error.message : String(error), { cause: "controller-capability", nextAction: "choose-application-mode" }); }
    return definition;
  };
  const profile = (model: GeneralModel, values: Readonly<Record<string, number>> = {}) => declaredCommissioningProfile(loop, model, requiredInitialization(), label, values);
  return { families,
    transferBinding: () => declaredTransferBinding(loop, requiredInitialization()),
    recoveryTracking: model => profile(model).tracking(loop, declaredTransferBinding(loop, requiredInitialization()), false),
    materialize: (gains, observed) => materializeBoundParameters(loop.parameterBinding!, symbols, gains.parameters ?? {}, observed),
    checkInitialization: () => { requiredInitialization(); },
    initialization: (model, task, values) => declaredInitializationPlan(loop, model, task, values, requiredInitialization()),
    assertRecoveryNeeded: (report, model) => {
      if (!model) throw new Error("Restore the commissioning model before recovery.");
      PidCommissioning.assertRecoveryNeeded(loop, report, profile(model));
    },
    commission: (model, task, client, review) => PidCommissioning.review(loop, model, task, client, review, profile(model, review.values)),
    recover: (model, task, client, review, report) => PidCommissioning.reviewRecovery(loop, model, task, client, review, report, profile(model)),
  };
}
