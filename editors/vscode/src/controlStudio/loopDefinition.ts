import { validPhysicalIdentification, type PhysicalIdentification } from "./physicalIdentification";
import { validPidControllerGroup, type PidControllerGroup } from "./pidControllerGroup";
import { decodeLibraryPidInitialization, libraryPidInitialization, type LibraryPidInitialization } from "./libraryPidInitialization";
import { decodeProjectController, type ProjectControllerBinding } from "./projectController";
import { decodeOnOffController, type OnOffControllerBinding } from "./onOffController";
import { decodeStateControllerBinding, type StateControllerBinding } from "./stateControllerDesign";
import { validIdentificationPolicy, type IdentificationPolicy } from "./identificationPolicy";
import { validEngineerScenarios, type EngineerScenario } from "./engineerVerificationCampaign";
import { validLoopChannels, validateLoopChannelBindings, type LoopChannels } from "./loopChannels";
import { decodeControllerLaw, decodePidLaw, type ControllerLaw } from "./controllerLaw";
import { decodeParameterBinding, validAnalysisParameterSymbols, type ParameterBinding } from "./parameterBinding";
import { decodeDeclaredAnalysis, type DeclaredControlAnalysis } from "./declaredAnalysis";
import { expectedPrimarySignalDirection, isAuxiliarySignalDirectionAccepted } from "./loopSignalDirections";

export type ControlSignalDirection = "read" | "write";

export interface ControlSignalBinding {
  symbol: string;
  address: string;
  iecType: string;
  direction: ControlSignalDirection;
  unit: string;
  range: { minimum: number; maximum: number };
}

export type ControlAuxiliarySignalRole =
  | "SP" | "PV" | "MV" | "actuator"
  | "disturbance" | "state" | "quality" | "schedule";

export interface ControlAuxiliarySignalBinding extends ControlSignalBinding {
  id: string;
  channel: string;
  role: ControlAuxiliarySignalRole;
}

/** Explicit runtime-state bindings needed to assess requested-versus-applied saturation. */
export interface ControlSaturationEvidence {
  requestedOutputId: string;
  integralStateId: string;
}

/** Explicit frozen inputs for one automatic isolated anti-windup comparison. */
export interface ControlAntiWindupVerificationScenario {
  stepMs: number;
  durationMs: number;
  seed: number;
  wallBudgetMs: number;
  initialValues: ReadonlyArray<{ symbol: string; value: number }>;
  events: ReadonlyArray<{ atMs: number; symbol: string; value: number }>;
}

/**
 * The actual retained states that a declared parallel-form PID implementation
 * needs at the Manual-to-Auto task boundary. These are state variables, not
 * parameters and therefore never travel through the parameter-record adapter.
 */
export interface ControlPidInitializationAdapter {
  kind: "parallel-pid-v1";
  integralState: { symbol: string; iecType: "REAL" | "LREAL"; unit: string; minimum: number; maximum: number };
  derivativeState: { symbol: string; iecType: "REAL" | "LREAL"; unit: string; minimum: number; maximum: number };
  proportionalSetpointWeight: number;
  derivativeSetpointWeight: number;
  maximumSnapshotAgeMs: number;
}

export interface ControlLoopDefinition {
  schemaVersion: 1 | 2;
  channels?: LoopChannels;
  physicalIdentification?: PhysicalIdentification;
  identificationPolicy?: IdentificationPolicy;
  verificationScenarios?: readonly EngineerScenario[];
  analysis?: DeclaredControlAnalysis;
  parameterBinding?: ParameterBinding;
  parameterPurpose?: "controller" | "experiment";
  analysisParameterSymbols?: Readonly<Record<string, string>>;
  topology?: "closed-loop" | "open-loop";
  id: string;
  name: string;
  revision: string;
  projectId: string;
  runtimeId: string;
  controller: {
    law?: ControllerLaw;
    stateFeedback?: StateControllerBinding;
    onOff?: OnOffControllerBinding;
    project?: ProjectControllerBinding;
    group?: PidControllerGroup;
    instance: string;
    type: string;
    design?: "pid" | "project";
    revision: string;
    action: "direct" | "reverse";
    taskPeriodMs: number;
    modeSignal?: string;
    modeWhenTrue?: "Auto" | "Manual";
    trackingSignal?: string;
    manualOutputSignal?: string;
    manualOutputAddress?: string;
    manualOutputType?: "REAL" | "LREAL" | "BOOL";
    pidInitialization?: ControlPidInitializationAdapter | LibraryPidInitialization;
    saturationEvidence?: ControlSaturationEvidence;
    antiWindupVerification?: ControlAntiWindupVerificationScenario;
  };
  signals: {
    setpoint?: ControlSignalBinding;
    processVariable: ControlSignalBinding;
    manipulatedVariable: ControlSignalBinding;
    actuator?: ControlSignalBinding;
    additional?: ReadonlyArray<ControlAuxiliarySignalBinding>;
  };
  limits: {
    outputMinimum: number;
    outputMaximum: number;
    rateUpPerS?: number;
    rateDownPerS?: number;
  };
  observedSampleTiming: {
    nominalPeriodMs: number;
    maximumJitterMs?: number;
  };
  experiment?: {
    execution: "control-studio" | "project-scripted";
    kind: "observation" | "setpoint_step" | "load_step" | "mv_step" | "mv_pulse" |
      "relay" | "prbs" | "multisine";
    amplitude: number;
    relayDirection?: "positive" | "negative";
    durationSeconds: number;
    baselineSeconds?: number;
    seed: number;
    activation?: {
      symbol: string;
      address: string;
      iecType: "BOOL";
      direction: "write";
    };
    abort?: {
      symbol: string;
      address: string;
      iecType: "BOOL";
      direction: "write";
      assertedValue: boolean;
    };
  };
  plantId?: string;
  operatingPoint?: string;
}

type PrimarySignalName = "setpoint" | "processVariable" | "manipulatedVariable" | "actuator";

export type ControlLoopDefinitionResult =
  | { kind: "accepted"; loop: ControlLoopDefinition }
  | { kind: "rejected"; reason: "malformed_loop" | "unsupported_version" };

export interface ControlLoopScope {
  projectId: string;
  runtimeId: string;
}

export type SharedPidDesignAvailability =
  | { available: true }
  | { available: false; reason: string };

/** Keeps the shared PID designer from being presented as the owner of project-defined control logic. */
export function sharedPidDesignAvailability(
  loop: ControlLoopDefinition
): SharedPidDesignAvailability {
  if ((loop.controller.design ?? "pid") === "pid") return { available: true };
  return {
    available: false,
    reason: `${loop.controller.type} is designed by the runnable project; shared Control.PID tuning is unavailable.`,
  };
}

export type ControlLoopValidationResult =
  | { kind: "accepted"; loop: ControlLoopDefinition }
  | { kind: "rejected"; reason: "malformed_loop" | "unsupported_version" }
  | { kind: "rejected"; reason: "cross_runtime_loop" }
  | { kind: "rejected"; reason: "incompatible_controller_design" }
  | { kind: "rejected"; reason: "incompatible_anti_windup_verification" }
  | { kind: "rejected"; reason: "incompatible_pid_initialization" }
  | { kind: "rejected"; reason: "incompatible_topology" }
  | { kind: "rejected"; reason: "incompatible_model_units" }
  | { kind: "rejected"; reason: "incompatible_saturation_evidence" }
  | {
      kind: "rejected";
      reason: "invalid_signal_direction" | "incompatible_signal_type";
      signal: keyof ControlLoopDefinition["signals"];
    }
  | {
      kind: "rejected";
      reason: "incompatible_units";
      signals: ["setpoint", "processVariable"];
    }
  | { kind: "rejected"; reason: "duplicate_signal_id" | "duplicate_signal_role"; signal: string }
  | { kind: "rejected"; reason: "ambiguous_signal_address"; signals: [string, string] }
  | { kind: "rejected"; reason: "incompatible_auxiliary_units"; channel: string };

/** Owns the serialized loop boundary before any runtime or UI state is joined. */
export function decodeControlLoopDefinition(value: unknown): ControlLoopDefinitionResult {
  if (!record(value)) return rejected("malformed_loop");
  if (value.schemaVersion !== 1 && value.schemaVersion !== 2) return rejected("unsupported_version");
  if (record(value.experiment) && (value.experiment.abort !== undefined || value.experiment.baselineSeconds !== undefined) && value.schemaVersion !== 2) return rejected("malformed_loop");
  if (value.parameterBinding !== undefined && (value.schemaVersion !== 2 || !decodeParameterBinding(value.parameterBinding, value.id))) return rejected("malformed_loop");
  if (value.parameterPurpose !== undefined) {
    const binding = decodeParameterBinding(value.parameterBinding, value.id);
    if (value.schemaVersion !== 2 || !binding || !["controller", "experiment"].includes(String(value.parameterPurpose))) return rejected("malformed_loop");
    if (value.parameterPurpose === "experiment" && (!record(value.experiment) || value.experiment.execution !== "project-scripted" ||
        !record(value.experiment.activation) || binding.mode.symbol !== value.experiment.activation.symbol || binding.mode.manual_value !== false)) return rejected("malformed_loop");
  }
  if (record(value.controller) && value.controller.law !== undefined &&
      (value.schemaVersion !== 2 || !decodeControllerLaw(value.controller.law, decodeParameterBinding(value.parameterBinding, value.id)))) return rejected("malformed_loop");
  if (value.verificationScenarios !== undefined && (value.schemaVersion !== 2 || !validEngineerScenarios(value.verificationScenarios))) return rejected("malformed_loop");
  if (value.physicalIdentification !== undefined && !validPhysicalIdentification(value.physicalIdentification)) return rejected("malformed_loop");
  if (value.identificationPolicy !== undefined && !validIdentificationPolicy(value.identificationPolicy)) return rejected("malformed_loop");
  if (value.channels !== undefined && (value.schemaVersion !== 2 || !validLoopChannels(value.channels))) return rejected("malformed_loop");
  if (record(value.controller) && value.controller.stateFeedback !== undefined &&
    (value.schemaVersion !== 2 || value.controller.design !== "project" || value.controller.law !== undefined
      || !decodeStateControllerBinding(value.controller.stateFeedback, decodeParameterBinding(value.parameterBinding, value.id)))) return rejected("malformed_loop");
  if (record(value.controller) && value.controller.onOff !== undefined &&
    (value.schemaVersion !== 2 || value.controller.design !== "project" || value.controller.law !== undefined || value.controller.stateFeedback !== undefined
      || !decodeOnOffController(value.controller.onOff, decodeParameterBinding(value.parameterBinding, value.id)))) return rejected("malformed_loop");
  if (record(value.controller) && value.controller.project !== undefined &&
    (value.schemaVersion !== 2 || value.controller.design !== "project" || value.controller.law !== undefined
      || value.controller.stateFeedback !== undefined || value.controller.onOff !== undefined
      || !decodeProjectController(value.controller.project, decodeParameterBinding(value.parameterBinding, value.id)))) return rejected("malformed_loop");
  if (value.analysis !== undefined && (value.schemaVersion !== 2 || !decodeDeclaredAnalysis(value.analysis))) return rejected("malformed_loop");
  if (value.analysisParameterSymbols !== undefined && (value.schemaVersion !== 2 || !validAnalysisParameterSymbols(
    value.analysisParameterSymbols, decodeParameterBinding(value.parameterBinding, value.id), decodeDeclaredAnalysis(value.analysis)))) return rejected("malformed_loop");
  if (
    !nonempty(value.id) || !nonempty(value.name) || !revision(value.revision) ||
    !nonempty(value.projectId) || !nonempty(value.runtimeId) ||
    !controller(value.controller) || !topology(value.topology) || !signals(value.signals, value.topology) ||
    !limits(value.limits) || !timing(value.observedSampleTiming) || !experiment(value.experiment) ||
    !optionalString(value.plantId) || !optionalString(value.operatingPoint)
  ) return rejected("malformed_loop");
  if (record(value.controller) && value.controller.group !== undefined && (value.schemaVersion !== 2
    || !validPidControllerGroup(value.controller.group, value.channels as LoopChannels | undefined))) return rejected("malformed_loop");
  return { kind: "accepted", loop: value as unknown as ControlLoopDefinition };
}

/**
 * Checks the relationships that cannot be established by decoding fields alone.
 * Rejecting an uncertain binding is safer than guessing which runtime value a
 * controller uses.
 */
export function validateControlLoopDefinition(
  value: unknown,
  scope: ControlLoopScope
): ControlLoopValidationResult {
  const decoded = decodeControlLoopDefinition(value);
  if (decoded.kind === "rejected") return decoded;
  const loop = decoded.loop;
  try { validateLoopChannelBindings(loop); } catch { return rejected("malformed_loop"); }
  if (loop.analysis && (loop.analysis.plant.inputUnit !== loop.signals.manipulatedVariable.unit ||
      loop.analysis.plant.outputUnit !== loop.signals.processVariable.unit)) {
    return { kind: "rejected", reason: "incompatible_model_units" };
  }
  if (loop.projectId !== scope.projectId || loop.runtimeId !== scope.runtimeId) {
    return { kind: "rejected", reason: "cross_runtime_loop" };
  }
  if ((loop.controller.design ?? "pid") === "pid" && loop.controller.type !== "Control.PID") {
    return { kind: "rejected", reason: "incompatible_controller_design" };
  }
  if (loop.controller.manualOutputType === "BOOL" &&
      (!loop.controller.onOff || loop.signals.manipulatedVariable.iecType.toUpperCase() !== "BOOL")) {
    return { kind: "rejected", reason: "incompatible_signal_type", signal: "manipulatedVariable" };
  }
  if (loop.topology === "open-loop" && (loop.controller.design ?? "pid") !== "project") {
    return { kind: "rejected", reason: "incompatible_topology" };
  }

  for (const name of ["setpoint", "processVariable", "manipulatedVariable", "actuator"] as const) {
    const binding = loop.signals[name];
    if (binding && binding.direction !== expectedPrimarySignalDirection(loop.experiment?.execution, name)) {
      return { kind: "rejected", reason: "invalid_signal_direction", signal: name };
    }
  }

  for (const name of ["setpoint", "processVariable", "manipulatedVariable", "actuator"] as const) {
    const binding = loop.signals[name];
    if (binding && !NUMERIC_IEC_TYPES.has(binding.iecType.toUpperCase())) {
      return { kind: "rejected", reason: "incompatible_signal_type", signal: name };
    }
  }
  if (loop.signals.setpoint && loop.signals.setpoint.unit !== loop.signals.processVariable.unit) {
    return {
      kind: "rejected",
      reason: "incompatible_units",
      signals: ["setpoint", "processVariable"],
    };
  }
  const additional = loop.signals.additional ?? [];
  const ids = new Set<string>(["setpoint", "processVariable", "manipulatedVariable", "actuator"]);
  const channelRoles = new Set<string>();
  const channelUnits = new Map<string, Partial<Record<"SP" | "PV", string>>>();
  for (const binding of additional) {
    if (ids.has(binding.id)) {
      return { kind: "rejected", reason: "duplicate_signal_id", signal: binding.id };
    }
    ids.add(binding.id);
    const channelRole = `${binding.channel}\0${binding.role}`;
    if (channelRoles.has(channelRole)) {
      return { kind: "rejected", reason: "duplicate_signal_role", signal: binding.id };
    }
    channelRoles.add(channelRole);
    const directionValid = isAuxiliarySignalDirectionAccepted(
      loop.experiment?.execution, binding.role, binding.direction
    );
    if (!directionValid) {
      return { kind: "rejected", reason: "invalid_signal_direction", signal: "additional" };
    }
    const bindingType = binding.iecType.toUpperCase();
    const booleanEvidence = bindingType === "BOOL" &&
      (binding.role === "quality" || binding.role === "state");
    if (!NUMERIC_IEC_TYPES.has(bindingType) && !booleanEvidence) {
      return { kind: "rejected", reason: "incompatible_signal_type", signal: "additional" };
    }
    if (binding.role === "SP" || binding.role === "PV") {
      const units = channelUnits.get(binding.channel) ?? {};
      units[binding.role] = binding.unit;
      channelUnits.set(binding.channel, units);
    }
  }
  for (const [channel, units] of channelUnits) {
    if (units.SP !== undefined && units.PV !== undefined && units.SP !== units.PV) {
      return { kind: "rejected", reason: "incompatible_auxiliary_units", channel };
    }
  }
  const saturation = saturationWindupEvidence(loop);
  if (saturation) {
    const requested = additional.find((binding) => binding.id === saturation.requestedOutputId);
    const integral = additional.find((binding) => binding.id === saturation.integralStateId);
    if (!requested || !integral || requested === integral || requested.role !== "state" || integral.role !== "state" ||
        requested.direction !== "read" || integral.direction !== "read" || requested.unit !== loop.signals.manipulatedVariable.unit ||
        integral.unit !== loop.signals.manipulatedVariable.unit) {
      return { kind: "rejected", reason: "incompatible_saturation_evidence" };
    }
  }
  const boundSignals: Array<[string, ControlSignalBinding]> = [
    ...(loop.signals.setpoint ? [["setpoint", loop.signals.setpoint] as [string, ControlSignalBinding]] : []),
    ["processVariable", loop.signals.processVariable],
    ["manipulatedVariable", loop.signals.manipulatedVariable],
    ...(loop.signals.actuator ? [["actuator", loop.signals.actuator] as [string, ControlSignalBinding]] : []),
    ...additional.map((binding) => [binding.id, binding] as [string, ControlSignalBinding]),
  ];
  for (let index = 0; index < boundSignals.length; index += 1) {
    for (let other = 0; other < index; other += 1) {
      if (addressRangesOverlap(boundSignals[index][1], boundSignals[other][1])) {
        return {
          kind: "rejected",
          reason: "ambiguous_signal_address",
          signals: [boundSignals[other][0], boundSignals[index][0]],
        };
      }
    }
  }
  if (loop.controller.antiWindupVerification && !validAntiWindupVerificationScenario(loop, saturation)) {
    return { kind: "rejected", reason: "incompatible_anti_windup_verification" };
  }
  if (loop.controller.pidInitialization && !validPidInitialization(loop)) {
    return { kind: "rejected", reason: "incompatible_pid_initialization" };
  }
  return { kind: "accepted", loop };
}

/** Returns only the declared stable IDs; live values remain runtime-record evidence. */
export function saturationWindupEvidence(loop: ControlLoopDefinition): ControlSaturationEvidence | undefined {
  const evidence = loop.controller.saturationEvidence;
  return evidence ? { requestedOutputId: evidence.requestedOutputId, integralStateId: evidence.integralStateId } : undefined;
}

function validAntiWindupVerificationScenario(loop: ControlLoopDefinition, saturation: ControlSaturationEvidence | undefined): boolean {
  const scenario = loop.controller.antiWindupVerification;
  const binding = loop.parameterBinding;
  const law = decodeControllerLaw(loop.controller.law, binding);
  if (!scenario || !binding || !law || law.kind !== "pid" || !law.antiWindup || !saturation || !loop.signals.setpoint) return false;
  const integral = loop.signals.additional?.find(signal => signal.id === saturation.integralStateId);
  if (!integral) return false;
  const initial = new Map(scenario.initialValues.map(value => [value.symbol, value.value]));
  const parameters = new Set(binding.parameters.map(parameter => parameter.symbol));
  return initial.get(binding.mode.symbol) === Number(!binding.mode.manual_value) && initial.has(loop.signals.setpoint.symbol) &&
    initial.has(integral.symbol) && !scenario.initialValues.some(value => parameters.has(value.symbol)) &&
    !scenario.events.some(value => parameters.has(value.symbol));
}

function validPidInitialization(loop: ControlLoopDefinition): boolean {
  const adapter = loop.controller.pidInitialization;
  const law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  const reference = loop.signals.setpoint;
  const binding = loop.parameterBinding;
  if (!adapter || !law || !reference || !binding) return false;
  if (adapter.kind === "control-pid-v1") {
    try { libraryPidInitialization(loop); return true; } catch { return false; }
  }
  const state = (value: { symbol: string; iecType: "REAL" | "LREAL"; unit: string; minimum: number; maximum: number }) =>
    nonempty(value.symbol) && (value.iecType === "REAL" || value.iecType === "LREAL") && typeof value.unit === "string" && value.unit.trim() !== "" &&
      finite(value.minimum) && finite(value.maximum) && value.minimum < value.maximum;
  const declaredParameters = new Set(binding.parameters.map(parameter => parameter.symbol));
  return state(adapter.integralState) && state(adapter.derivativeState) &&
    adapter.integralState.symbol !== adapter.derivativeState.symbol &&
    !declaredParameters.has(adapter.integralState.symbol) && !declaredParameters.has(adapter.derivativeState.symbol) &&
    adapter.integralState.symbol !== reference.symbol && adapter.derivativeState.symbol !== reference.symbol &&
    adapter.integralState.symbol !== loop.signals.processVariable.symbol && adapter.derivativeState.symbol !== loop.signals.processVariable.symbol &&
    adapter.integralState.symbol !== loop.signals.manipulatedVariable.symbol && adapter.derivativeState.symbol !== loop.signals.manipulatedVariable.symbol &&
    adapter.integralState.unit === loop.signals.manipulatedVariable.unit && adapter.derivativeState.unit === reference.unit &&
    adapter.proportionalSetpointWeight >= 0 && adapter.proportionalSetpointWeight <= 1 &&
    adapter.derivativeSetpointWeight >= 0 && adapter.derivativeSetpointWeight <= 1 &&
    Number.isInteger(adapter.maximumSnapshotAgeMs) && adapter.maximumSnapshotAgeMs >= loop.controller.taskPeriodMs && adapter.maximumSnapshotAgeMs <= 10_000;
}

const NUMERIC_IEC_TYPES = new Set([
  "SINT", "INT", "DINT", "LINT",
  "USINT", "UINT", "UDINT", "ULINT",
  "REAL", "LREAL",
]);

function controller(value: unknown): boolean {
  return record(value) && nonempty(value.instance) && nonempty(value.type) &&
    (value.design === undefined || value.design === "pid" || value.design === "project") &&
    revision(value.revision) && (value.action === "direct" || value.action === "reverse") &&
    positive(value.taskPeriodMs) && optionalString(value.modeSignal) &&
    (value.modeWhenTrue === undefined || value.modeWhenTrue === "Auto" || value.modeWhenTrue === "Manual") &&
    optionalString(value.trackingSignal) && optionalString(value.manualOutputSignal) && optionalString(value.manualOutputAddress) &&
    (value.manualOutputType === undefined || nonempty(value.manualOutputSignal) &&
      (value.manualOutputType === "REAL" || value.manualOutputType === "LREAL" || value.manualOutputType === "BOOL")) &&
    saturationEvidence(value.saturationEvidence) && antiWindupVerification(value.antiWindupVerification) && pidInitialization(value.pidInitialization);
}

function saturationEvidence(value: unknown): boolean {
  return value === undefined || record(value) && Object.keys(value).sort().join(",") === "integralStateId,requestedOutputId" &&
    nonempty(value.requestedOutputId) && nonempty(value.integralStateId) && value.requestedOutputId !== value.integralStateId;
}

function antiWindupVerification(value: unknown): boolean {
  if (value === undefined) return true;
  if (!record(value) || Object.keys(value).sort().join(",") !== "durationMs,events,initialValues,seed,stepMs,wallBudgetMs") return false;
  const { stepMs, durationMs, seed, wallBudgetMs, initialValues, events: configuredEvents } = value;
  if (!positiveInteger(stepMs) || !positiveInteger(durationMs) || durationMs % stepMs !== 0 || !safeInteger(seed) || seed < 0 ||
      !positiveInteger(wallBudgetMs) || wallBudgetMs > 10_000 || !Array.isArray(initialValues) || initialValues.length === 0 ||
      initialValues.length > 64 || !Array.isArray(configuredEvents) || configuredEvents.length > 1024) return false;
  const initialSymbols = new Set<string>();
  if (!initialValues.every(item => record(item) && Object.keys(item).sort().join(",") === "symbol,value" &&
      nonempty(item.symbol) && finite(item.value) && !initialSymbols.has(item.symbol) && (initialSymbols.add(item.symbol), true))) return false;
  let previous = 0;
  const events = new Set<string>();
  return configuredEvents.every(item => record(item) && Object.keys(item).sort().join(",") === "atMs,symbol,value" &&
    safeInteger(item.atMs) && item.atMs >= previous && item.atMs <= durationMs && item.atMs % stepMs === 0 &&
    nonempty(item.symbol) && finite(item.value) && !events.has(`${item.atMs}\0${item.symbol}`) &&
    (events.add(`${item.atMs}\0${item.symbol}`), previous = item.atMs, true));
}

function pidInitialization(value: unknown): boolean {
  if (value === undefined) return true;
  if (record(value) && value.kind === "control-pid-v1") return !!decodeLibraryPidInitialization(value);
  if (!record(value) || Object.keys(value).sort().join(",") !== "derivativeSetpointWeight,derivativeState,integralState,kind,maximumSnapshotAgeMs,proportionalSetpointWeight" ||
      value.kind !== "parallel-pid-v1" || !pidInitializationState(value.integralState) || !pidInitializationState(value.derivativeState)) return false;
  return finite(value.proportionalSetpointWeight) && finite(value.derivativeSetpointWeight) && safeInteger(value.maximumSnapshotAgeMs);
}

function pidInitializationState(value: unknown): boolean {
  return record(value) && Object.keys(value).sort().join(",") === "iecType,maximum,minimum,symbol,unit" && nonempty(value.symbol) &&
    (value.iecType === "REAL" || value.iecType === "LREAL") && typeof value.unit === "string" && value.unit.trim() !== "" &&
    finite(value.minimum) && finite(value.maximum) && value.minimum < value.maximum;
}

function topology(value: unknown): boolean {
  return value === undefined || value === "closed-loop" || value === "open-loop";
}

function signals(value: unknown, declaredTopology: unknown): boolean {
  if (!record(value)) return false;
  const openLoop = declaredTopology === "open-loop";
  return (openLoop ? value.setpoint === undefined : signal(value.setpoint)) &&
    signal(value.processVariable) &&
    signal(value.manipulatedVariable) && (value.actuator === undefined || signal(value.actuator)) &&
    (value.additional === undefined || (Array.isArray(value.additional) && value.additional.every(auxiliarySignal)));
}

function auxiliarySignal(value: unknown): boolean {
  return record(value) && signal(value) && nonempty(value.id) && nonempty(value.channel) &&
    ["SP", "PV", "MV", "actuator", "disturbance", "state", "quality", "schedule"].includes(String(value.role));
}

function signal(value: unknown): boolean {
  return record(value) && nonempty(value.symbol) && nonempty(value.address) &&
    addressRange(value.address as string) !== undefined &&
    nonempty(value.iecType) && (value.direction === "read" || value.direction === "write") &&
    typeof value.unit === "string" && range(value.range);
}

function range(value: unknown): boolean {
  return record(value) && finite(value.minimum) && finite(value.maximum) && value.minimum < value.maximum;
}

function addressRangesOverlap(left: ControlSignalBinding, right: ControlSignalBinding): boolean {
  const leftRange = addressRange(left.address);
  const rightRange = addressRange(right.address);
  if (!leftRange || !rightRange || leftRange.area !== rightRange.area) return left.address === right.address;
  return leftRange.startBit < rightRange.endBit && rightRange.startBit < leftRange.endBit;
}

function addressRange(address: string): { area: string; startBit: number; endBit: number } | undefined {
  const match = /^%([IQM])([XBWDL])(\d+)(?:\.(\d+))?$/.exec(address.toUpperCase());
  if (!match) return undefined;
  const byte = Number(match[3]);
  const bit = match[2] === "X" ? Number(match[4] ?? "0") : 0;
  if (!Number.isSafeInteger(byte) || !Number.isSafeInteger(bit) || bit < 0 || bit > 7) return undefined;
  const width = ({ X: 1, B: 8, W: 16, D: 32, L: 64 } as const)[match[2] as "X" | "B" | "W" | "D" | "L"];
  const startBit = byte * 8 + bit;
  return { area: match[1], startBit, endBit: startBit + width };
}

function limits(value: unknown): boolean {
  return record(value) && finite(value.outputMinimum) && finite(value.outputMaximum) &&
    value.outputMinimum < value.outputMaximum && optionalNonnegative(value.rateUpPerS) &&
    optionalNonnegative(value.rateDownPerS);
}

function timing(value: unknown): boolean {
  return record(value) && positive(value.nominalPeriodMs) && optionalNonnegative(value.maximumJitterMs);
}

function experiment(value: unknown): boolean {
  if (value === undefined) return true;
  if (!record(value) || (value.execution !== "control-studio" && value.execution !== "project-scripted") ||
      !["observation", "setpoint_step", "load_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine"]
        .includes(String(value.kind)) ||
      !finite(value.amplitude) || !finite(value.durationSeconds) || value.durationSeconds < 1 ||
      value.durationSeconds > 3_600 || typeof value.seed !== "number" || !Number.isSafeInteger(value.seed) ||
      value.seed < 1 || value.seed > 0xffff_ffff) return false;
  if (value.baselineSeconds !== undefined && (value.execution !== "project-scripted" || !activation(value.activation) ||
      !finite(value.baselineSeconds) || value.baselineSeconds < 0 || value.baselineSeconds >= value.durationSeconds)) return false;
  if (value.execution === "project-scripted") {
    return value.kind === "observation" && value.amplitude === 0 && (value.activation === undefined || activation(value.activation)) &&
      (value.abort === undefined || (activation(value.activation) && activation(value.abort) && record(value.abort) &&
        typeof value.abort.assertedValue === "boolean" && record(value.activation) &&
        value.abort.address !== value.activation.address && value.abort.symbol !== value.activation.symbol));
  }
  return value.activation === undefined && value.abort === undefined &&
    (value.kind === "observation" ? value.amplitude === 0 : value.amplitude !== 0);
}

function activation(value: unknown): boolean {
  return record(value) && nonempty(value.symbol) && nonempty(value.address) &&
    addressRange(value.address as string) !== undefined && value.iecType === "BOOL" &&
    value.direction === "write";
}

function rejected(reason: "malformed_loop" | "unsupported_version"): ControlLoopDefinitionResult {
  return { kind: "rejected", reason };
}

function record(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function nonempty(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function optionalString(value: unknown): boolean {
  return value === undefined || nonempty(value);
}

function revision(value: unknown): value is string {
  return nonempty(value) && value.startsWith("sha256:") && value.length > "sha256:".length;
}

function finite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function safeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value);
}

function positiveInteger(value: unknown): value is number {
  return safeInteger(value) && value > 0;
}

function positive(value: unknown): value is number {
  return finite(value) && value > 0;
}

function optionalNonnegative(value: unknown): boolean {
  return value === undefined || (finite(value) && value >= 0);
}
