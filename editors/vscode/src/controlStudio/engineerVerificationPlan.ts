import { pidOperatingState } from "./pidOperatingState";
import { measuredPid, plantFeedbackTarget } from "./pidMeasurement";
import { decodeAutomaticTestCriteria, type AutomaticTestCriteria } from "./automaticTestCriteria";
import { libraryPidInitialization } from "./libraryPidInitialization";
import { initializationValues } from "./controllerInitialState";
import { connectedModelChannels, validChannelRequirements, type ChannelRequirements } from "./loopChannels";
import { assertEngineerModelCompatibility } from "./engineerModelCompatibility";
import { decodeProportionalLaw } from "./controllerLaw";
import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { ParameterBinding } from "./parameterBinding";
import { materializeSelectedModelSimulation } from "./modelSimulation";
import type { RuntimeVerificationRequest } from "./verificationClient";
import { isolatedRecordEvery } from "./isolatedRunSampling";
import type { RunRequirement } from "./runValidation";
import { ISOLATED_SIMULATION_DURATION_LIMIT_MS, ISOLATED_SIMULATION_SAMPLE_LIMIT, ISOLATED_SIMULATION_SCAN_LIMIT, ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS, MAXIMUM_EXPERIMENT_DURATION_MS, recordPeriodExecutions } from "./controlStudioLimits";

export interface EngineerRequirements {
  automaticCriteria?: AutomaticTestCriteria;
  constantInputDisturbance?: number;
  measurementNoiseBand?: number;
  gainUncertaintyFraction?: number;
  referenceChange: number;
  maximumError: number;
  settlingBand: number;
  settlingTimeSeconds: number;
  /** Percent of the setpoint change; absent declares no overshoot limit (Specification 35 B4.3). */
  maximumOvershootPercent?: number;
  durationSeconds: number;
  transferTolerance?: number;
  tuningMethod?: "SIMC" | "AMIGO";
  tuningObjective?: "balanced" | "robust" | "fast";
  channelRequirements?: Readonly<Record<string, ChannelRequirements>>;
}

/** The largest declared overshoot limit, in percent of the setpoint change. */
export const MAXIMUM_OVERSHOOT_PERCENT = 1000;

export function engineerRequirements(value: Record<string, unknown>): EngineerRequirements {
  const keys = ["referenceChange", "maximumError", "settlingBand", "settlingTimeSeconds", "durationSeconds"] as const;
  if (!keys.every(key => typeof value[key] === "number" && Number.isFinite(value[key]))) throw new Error("Enter finite task requirements.");
  const result = Object.fromEntries(keys.map(key => [key, value[key]])) as unknown as EngineerRequirements;
  if (!result.referenceChange || result.settlingBand <= 0 || result.maximumError <= result.settlingBand
      || result.settlingTimeSeconds <= 0 || result.durationSeconds <= result.settlingTimeSeconds || result.durationSeconds > MAXIMUM_EXPERIMENT_DURATION_MS / 1_000) {
    throw new Error("Use a nonzero target change, positive response time, a smaller settling allowance than the error limit, and a longer test duration.");
  }
  if (value.transferTolerance !== undefined) {
    if (typeof value.transferTolerance !== "number" || !Number.isFinite(value.transferTolerance) || value.transferTolerance < 0) {
      throw new Error("Auto-entry tolerance must be blank for automatic or a finite nonnegative value in output units.");
    }
    result.transferTolerance = value.transferTolerance;
  }
  if (value.tuningMethod !== undefined) {
    if (value.tuningMethod !== "SIMC" && value.tuningMethod !== "AMIGO") throw new Error("Choose a supported tuning method.");
    result.tuningMethod = value.tuningMethod;
  }
  if (value.tuningObjective !== undefined) {
    if (value.tuningObjective !== "balanced" && value.tuningObjective !== "robust" && value.tuningObjective !== "fast") throw new Error("Choose a supported tuning objective.");
    result.tuningObjective = value.tuningObjective;
  }
  const maximumOvershootPercent = decodeMaximumOvershootPercent(value.maximumOvershootPercent, "Maximum overshoot");
  if (maximumOvershootPercent !== undefined) result.maximumOvershootPercent = maximumOvershootPercent;
  for (const key of ["constantInputDisturbance", "measurementNoiseBand", "gainUncertaintyFraction"] as const) {
    if (value[key] === undefined) continue;
    const bound = value[key];
    if (typeof bound !== "number" || !Number.isFinite(bound) || key !== "constantInputDisturbance" && bound < 0
      || key === "gainUncertaintyFraction" && bound > 1) throw new Error("Enter finite disturbance/noise bounds and gain uncertainty between zero and one.");
    result[key] = bound;
  }
  const channelRequirements = decodeChannelRequirements(value.channelRequirements);
  if (value.channelRequirements !== undefined && !channelRequirements) throw new Error("Enter valid per-output control requirements.");
  if (channelRequirements) {
    const slowest = Math.max(...Object.values(channelRequirements).map(requirement => requirement.settlingTimeSeconds));
    if (result.durationSeconds <= slowest) throw new Error("Allow longer than every channel settling time.");
    result.channelRequirements = channelRequirements;
  }
  if (value.automaticCriteria !== undefined) {
    const criteria = decodeAutomaticTestCriteria(value.automaticCriteria);
    if (!criteria) throw new Error("Invalid automatic test criteria.");
    result.automaticCriteria = criteria;
  }
  return result;
}

function decodeChannelRequirements(value: unknown): Record<string, ChannelRequirements> | undefined {
  if (value === undefined) return undefined;
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const entries = Object.entries(value);
  if (entries.length === 0 || entries.length > 16 || entries.some(([key, requirement]) => !key.trim() || !validChannelRequirements(requirement))) return undefined;
  // An output's own limit is named by that output, so an invalid one says where it is.
  for (const [key, requirement] of entries) decodeMaximumOvershootPercent(requirement.maximumOvershootPercent, `Maximum overshoot for ${key}`);
  return Object.fromEntries(entries) as Record<string, ChannelRequirements>;
}

/** A declared overshoot limit in percent of the setpoint change; a blank value declares no limit (Specification 35 B4.3). */
function decodeMaximumOvershootPercent(value: unknown, label: string): number | undefined {
  if (value === undefined || value === null || typeof value === "string" && value.trim() === "") return undefined;
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0 || value > MAXIMUM_OVERSHOOT_PERCENT) {
    throw new Error(`${label} must be blank for no limit, or a finite value above 0 and at most ${MAXIMUM_OVERSHOOT_PERCENT} % of the setpoint change.`);
  }
  return value;
}

/** Uses an explicitly declared existing P law; variable names never imply control semantics. */
export function proportionalParameter(loop: ControlLoopDefinition): ParameterBinding["parameters"][number] {
  if (loop.controller.law !== undefined) {
    const law = decodeProportionalLaw(loop.controller.law, loop.parameterBinding);
    if (!law || !loop.signals.setpoint) throw new Error("Confirm the proportional law, gain binding and setpoint before tuning.");
    return loop.parameterBinding!.parameters.find(parameter => parameter.symbol === law.gainSymbol)!;
  }
  const controller = loop.analysis?.controller;
  const coefficient = controller?.numerator[0];
  if (!controller || controller.numerator.length !== 1 || controller.denominator.length !== 1
      || controller.denominator[0] !== 1 || typeof coefficient !== "object" || (coefficient.scale ?? 1) !== 1) {
    throw new Error("This connected design adapter requires an explicitly declared proportional ST controller. Other controller adapters remain available through their supported manual operations.");
  }
  const symbol = loop.analysisParameterSymbols?.[coefficient.parameter];
  const parameter = loop.parameterBinding?.parameters.find(parameter => parameter.symbol === symbol);
  if (!parameter || !loop.signals.setpoint) throw new Error("Confirm the proportional parameter, manual-mode binding and setpoint before tuning this ST loop.");
  return parameter;
}

/** The response deadline: the settling time rounded up to a whole task period. */
export function responseDeadlineSeconds(settlingTimeSeconds: number, stepMs: number): number {
  return Math.ceil(settlingTimeSeconds * 1000 / stepMs) * stepMs / 1000;
}

export function engineerVerificationPlan(
  loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements, values: Readonly<Record<string, number>>,
  initialModelState?: readonly number[]
): { request: RuntimeVerificationRequest; requirements: readonly RunRequirement[]; coordinates: string } {
  const binding = loop.parameterBinding;
  const reference = loop.signals.setpoint;
  if (!binding || !reference) throw new Error("Declare the parameter and reference bindings first.");
  assertEngineerModelCompatibility(loop, model);
  const channels = connectedModelChannels(loop, model);
  const step = loop.controller.taskPeriodMs;
  if (!Number.isInteger(step) || step < 1 || step > 1000) throw new Error("This runtime adapter requires a whole-millisecond task period from 1 to 1000.");
  const duration = Math.ceil(task.durationSeconds * 1000 / step) * step;
  const settled = responseDeadlineSeconds(task.settlingTimeSeconds, step);
  // A long run is recorded with a record period, so its samples fit the run's sample and channel-sample limits (A7, A4.2;
  // verificationClient.ts isolatedRecordEvery). Only its duration and its scans per job are bounded here.
  if (duration > ISOLATED_SIMULATION_DURATION_LIMIT_MS || settled >= duration / 1000) {
    throw new Error("Choose a validation of at most 24 h that runs past the response deadline.");
  }
  if (duration / step + 1 > ISOLATED_SIMULATION_SCAN_LIMIT) {
    throw new Error(`Validation would execute ${duration / step + 1} scans; the runtime executes at most ${ISOLATED_SIMULATION_SCAN_LIMIT} per run. Shorten the test duration.`);
  }
  const states = model.kind === "transfer-function" ? model.denominator.length - 1 : model.states.length;
  const initialInputs = channels.inputs.map((input, index) => {
    const origin = model.operatingPoint?.inputs[index] ?? 0;
    return input.actuator.kind === "on-off" && origin !== input.actuator.maximum ? input.actuator.minimum : origin;
  });
  const materialized = materializeSelectedModelSimulation(model, {
    // A signal without an address is the program variable itself (Specification 33 §13.1–§13.2); a PID's own measurement is
    // given to that PID as what its Update receives, whatever the program computes it from.
    plantId: "selected-model", commandSources: channels.inputs.map(input => input.signal.address ?? input.signal.symbol),
    feedbackTargets: channels.outputs.map(output => plantFeedbackTarget(loop, output.signal)), actuatorFeedbackTargets: channels.inputs.map(input => input.applied?.address && !channels.outputs.some(output => output.signal.address === input.applied!.address) ? input.applied.address : null), initialState: initialModelState ?? channels.initialState ?? Array<number>(states).fill(0),
    initialInput: initialInputs.map((value, index) => value - (model.operatingPoint?.inputs[index] ?? 0)),
    initialDelayedInput: initialInputs[0] - (model.operatingPoint?.inputs[0] ?? 0), stepMs: step,
    actuators: channels.inputs.map(input => input.actuator),
  });
  const initialization = loop.controller.project?.initialization ?? loop.controller.onOff?.initialization
    ?? loop.controller.stateFeedback?.initialization
    ?? (loop.controller.pidInitialization?.kind === "control-pid-v1" ? libraryPidInitialization(loop) : undefined);
  const primary = [...channels.inputs.flatMap(input => input.applied ? [input.signal, input.applied] : [input.signal]), ...channels.outputs.flatMap(output =>
    output.reference ? [output.signal, output.reference] : [output.signal]), ...(loop.signals.additional ?? [])];
  const observations = [...new Map([...primary, ...binding.parameters, ...(initialization?.states ?? [])].map(signal =>
    [signal.symbol, { symbol: signal.symbol, unit: signal.unit }])).values()];
  if (observations.length > 64) throw new Error("Verification supports at most 64 observed channels, including parameters.");
  for (const parameter of binding.parameters) {
    const value = values[parameter.symbol];
    if (!Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum
      || parameter.iec_type === "REAL" && Math.fround(value) !== value) throw new Error(`Parameter ${parameter.symbol} is missing, out of bounds or not representable.`);
  }
  const initializationModel = { ...model, operatingPoint: { inputs: initialInputs,
    outputs: model.operatingPoint?.outputs ?? model.outputs.map(() => 0) } };
  const initialValues: Record<string, number> = {
    ...pidOperatingState(loop, initializationModel, values),
    ...(initialization ? initializationValues(loop, initializationModel, initialization, values) : {}),
    [binding.mode.symbol]: Number(!binding.mode.manual_value),
    // Further controllers switched with the loop, such as a cascade's outer one, run in Auto too (Specification 35 §17.11).
    ...Object.fromEntries((loop.controller.project?.additionalModes ?? []).map(mode => [mode.symbol, Number(!mode.manualValue)])),
  };
  // A PID's own measurement is both one of its states and the observed measurement: the model gives it to the PID.
  if (initialization?.states.some(state => binding.parameters.some(parameter => parameter.symbol === state.symbol)
    || [...channels.inputs.map(input => input.signal), ...channels.outputs.flatMap(output => output.reference ? [output.signal, output.reference] : [output.signal])]
      .some(signal => signal.symbol === state.symbol && measuredPid(loop, signal.symbol) === undefined) || state.symbol === binding.mode.symbol)) {
    throw new Error("Controller initial states cannot overlap parameters, observed process channels or mode ownership.");
  }
  for (const [index, input] of channels.inputs.entries()) initialValues[input.signal.symbol] = initialInputs[index];
  const window = { start: 0, end: duration / 1000 };
  const requirements: RunRequirement[] = (initialization?.states ?? []).map(state => ({
    id: `controller-state:${state.symbol}`, kind: "state-bounds", channel: state.symbol, window,
    minimum: state.postScan === "unchanged" ? initialValues[state.symbol] : state.minimum,
    maximum: state.postScan === "unchanged" ? initialValues[state.symbol] : state.maximum,
  }));
  for (const [index, output] of channels.outputs.entries()) {
    const prefix = channels.outputs.length === 1 ? "" : `${model.outputs[index].id}:`;
    const required = output.requirements ?? (output.signal.symbol === loop.signals.processVariable.symbol ? task : undefined);
    if (output.reference && !required) throw new Error(`Declare requirements for controlled output ${model.outputs[index].id}.`);
    if (required && output.reference) {
      const target = (model.operatingPoint?.outputs[index] ?? 0) + required.referenceChange;
      const deadline = responseDeadlineSeconds(required.settlingTimeSeconds, step);
      if (deadline >= window.end || target < output.signal.range.minimum || target > output.signal.range.maximum
        || target < output.reference.range.minimum || target > output.reference.range.maximum
        || target < model.outputs[index].operatingRange.minimum || target > model.outputs[index].operatingRange.maximum) throw new Error(`Target or deadline is outside the declared scope for ${output.signal.symbol}.`);
      initialValues[output.reference.symbol] = target;
      // A cascade's inner setpoint is the outer output; it holds the target only with the outer PID in Manual.
      const hold = loop.cascade?.referenceHold;
      if (hold && output.reference.symbol === loop.signals.setpoint?.symbol) {
        initialValues[hold.modeSymbol] = Number(hold.manualValue);
        initialValues[hold.manualOutputSymbol] = target;
      }
      requirements.push({ id: `${prefix}response-deadline`, kind: "settling", channel: output.signal.symbol,
        target, band: required.maximumError, deadlineSeconds: deadline, holdSeconds: window.end - deadline, window },
      { id: `${prefix}target-error`, kind: "error", channel: output.signal.symbol, reference: { channel: output.reference.symbol },
        statistic: "maximum", maximum: required.maximumError, window: { start: deadline, end: window.end } });
      // A declared overshoot limit bounds the peak of this setpoint step, in output units. A run that applies no step declares none.
      const overshootPercent = (required as ChannelRequirements & Pick<EngineerRequirements, "maximumOvershootPercent">).maximumOvershootPercent;
      if (overshootPercent !== undefined && required.referenceChange !== 0) {
        requirements.push({ id: `${prefix}overshoot`, kind: "overshoot", channel: output.signal.symbol, window,
          initial: model.operatingPoint?.outputs[index] ?? 0, target, maximum: overshootPercent / 100 * Math.abs(required.referenceChange) });
      }
    }
    requirements.push({ id: `${prefix}process-bounds`, kind: "bounds", channel: output.signal.symbol, window,
      minimum: Math.max(output.signal.range.minimum, model.outputs[index].operatingRange.minimum),
      maximum: Math.min(output.signal.range.maximum, model.outputs[index].operatingRange.maximum) });
  }
  // A measured disturbance is not a controller output: scenarios move it and nothing bounds it as one (Specification 35 §17.12).
  const actuated = channels.inputs.filter(input => !input.disturbance).length;
  for (const [index, input] of channels.inputs.entries()) {
    if (input.disturbance) continue;
    const prefix = actuated === 1 ? "" : `${model.inputs[index].id}:`;
    const applied = input.applied ?? input.signal;
    requirements.push({ id: `${prefix}output-bounds`, kind: "bounds", channel: applied.symbol, window,
      minimum: Math.max(input.actuator.minimum, model.inputs[index].operatingRange.minimum),
      maximum: Math.min(input.actuator.maximum, model.inputs[index].operatingRange.maximum) });
    if (input.applied) requirements.push({ id: `${prefix}requested-output-bounds`, kind: "bounds", channel: input.signal.symbol, window,
      minimum: input.signal.range.minimum, maximum: input.signal.range.maximum });
    if (input.actuator.kind === "on-off") {
      requirements.push({ id: `${prefix}actuator-levels`, kind: "levels", channel: applied.symbol, window,
        allowed: [input.actuator.minimum, input.actuator.maximum], tolerance: 0 });
      for (const [state, minimumSeconds] of [["high", input.actuator.minimumOnSeconds], ["low", input.actuator.minimumOffSeconds]] as const) {
        if (minimumSeconds !== undefined && minimumSeconds > 0) requirements.push({ id: `${prefix}actuator-${state}-dwell`, kind: "dwell",
          channel: applied.symbol, window, threshold: (input.actuator.minimum + input.actuator.maximum) / 2, state, minimumSeconds });
      }
    }
    for (const direction of ["up", "down"] as const) {
      const maximum = direction === "up" ? input.actuator.rateUpPerS : input.actuator.rateDownPerS;
      if (maximum !== undefined) requirements.push({ id: `${prefix}output-rate-${direction}`, kind: "rate", direction,
        channel: applied.symbol, window, maximum });
    }
  }
  if (loop.controller.onOff) {
    const switching = loop.controller.onOff;
    const threshold = (switching.lowOutput + switching.highOutput) / 2;
    requirements.push({ id: "switching-output-levels", kind: "levels", channel: loop.signals.manipulatedVariable.symbol,
      window, allowed: [switching.lowOutput, switching.highOutput], tolerance: 0 });
    requirements.push({ id: "switching-on-dwell", kind: "dwell", channel: loop.signals.manipulatedVariable.symbol,
      window, threshold, state: "high", minimumSeconds: values[switching.minimumOnTimeSymbol] });
    requirements.push({ id: "switching-off-dwell", kind: "dwell", channel: loop.signals.manipulatedVariable.symbol,
      window, threshold, state: "low", minimumSeconds: values[switching.minimumOffTimeSymbol] });
  }
  requirements.push(...binding.parameters.map(parameter => ({ id: `fixed-parameter:${parameter.symbol}`, kind: "bounds" as const,
    channel: parameter.symbol, window, minimum: values[parameter.symbol], maximum: values[parameter.symbol] })));
  // One check per condition: an ON/OFF controller's switching levels and dwell times can repeat its on-off output's.
  const condition = (requirement: RunRequirement) => JSON.stringify(Object.entries(requirement).filter(([key]) => key !== "id")
    .sort(([left], [right]) => left.localeCompare(right)));
  const unique = requirements.filter((requirement, index) => requirements.findIndex(other => condition(other) === condition(requirement)) === index);
  const request: RuntimeVerificationRequest = { step_ms: step, duration_ms: duration, seed: loop.experiment?.seed ?? 1,
    max_total_cycles: duration / step + 1, wall_budget_ms: ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS,
    initial_values: initialValues,
    events: [], observations, simulation_override: materialized.simulation_override,
    parameter_overrides: binding.parameters.map(parameter => ({ ...parameter, value: values[parameter.symbol] })),
  };
  // The controller runs at every execution; the run is recorded at a record period that follows the process (A4.2). An ON/OFF
  // output's levels and dwell times are checked at every execution, so its run keeps them all.
  const every = Math.max(isolatedRecordEvery(request), loop.controller.onOff ? 1
    : recordPeriodExecutions(duration, step, ISOLATED_SIMULATION_SAMPLE_LIMIT, task.settlingTimeSeconds));
  return { coordinates: materialized.stateCoordinateDescription,
    request: every > 1 ? { ...request, record_every: every } : request, requirements: unique,
  };
}
