import type { ControlLoopDefinition } from "../loopDefinition";
import type { FeedforwardPidSeed } from "../feedforwardPidDesign";
import { PID_TYPE, type CompiledCatalog } from "./compiledCatalog";
import { binding, compiled, explicitExperiment, loopId, measurementBinding, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { pidParameters, pidStates, startupState, manualOutputTracking } from "./pidBindings";
import type { ModeChoice, SignalChoice } from "./pidLoopGenerator";

/** A `Control.PID` whose program adds a feedforward from a measured disturbance to its `FeedForward` input (Specification 35 §17.12). */
export interface FeedforwardDraft {
  projectId: string; runtimeId: string; name: string;
  instance: string; action: "direct" | "reverse"; parameterRecord: string;
  processVariable: MeasuredSignal; manipulatedVariable: MeasuredSignal; setpoint: SignalChoice;
  mode: ModeChoice; manualOutput: string;
  /** The measured disturbance the program's feedforward reads. */
  disturbance: MeasuredSignal;
  /** The variable Identify steps to move the disturbance, in a simulation usually the simulated load itself. */
  disturbanceCommand: string;
  /** The disturbance's identification step, also the step Validate rejects. */
  disturbanceStep: number;
  /** The program's gain variable and, all three or none, its lead time, lag time and the filtered disturbance it keeps. */
  feedforward: { gain: string; lead?: string; lag?: string; filtered?: string };
  /** The controller output's identification step; a tenth of the output range when absent. */
  experimentStep?: number;
  experimentDurationSeconds?: number;
  startupFlag?: string;
}

/** One loop whose model has the controller output and the measured disturbance as inputs and the measurement as output. */
export function feedforwardLoopDefinition(catalog: CompiledCatalog, draft: FeedforwardDraft): ControlLoopDefinition {
  const controller = catalog.controllers.find(entry => entry.instance === draft.instance && entry.type.toUpperCase() === PID_TYPE);
  if (!controller) throw new Error(`${draft.instance} is not a compiled Control.PID instance.`);
  if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${draft.instance} must run in exactly one cyclic task.`);
  const task = controller.tasks[0], period = task.intervalMs, id = loopId(draft.name);
  const pv = draft.processVariable, mv = draft.manipulatedVariable, disturbance = draft.disturbance;
  const { gain, lead, lag, filtered } = draft.feedforward;
  const leadLag = [lead, lag, filtered];
  if (leadLag.some(Boolean) && !leadLag.every(Boolean)) throw new Error("Choose the lead time, the lag time and the filtered disturbance together, or none of them.");
  const owned = [gain, ...leadLag.filter((symbol): symbol is string => !!symbol)];
  if (new Set([...owned, pv.symbol, mv.symbol, disturbance.symbol].map(symbol => symbol.toUpperCase())).size !== owned.length + 3) {
    throw new Error("The feedforward's variables must differ from each other and from the loop's signals.");
  }
  const span = disturbance.maximum - disturbance.minimum;
  if (!(draft.disturbanceStep > 0) || draft.disturbanceStep > span) throw new Error(`The disturbance step must be positive and at most ${span} ${disturbance.unit}.`);
  const setpoint: MeasuredSignal = { symbol: draft.setpoint.symbol, unit: draft.setpoint.unit ?? pv.unit,
    minimum: draft.setpoint.minimum ?? pv.minimum, maximum: draft.setpoint.maximum ?? pv.maximum };
  const manual = compiled(catalog, draft.manualOutput), command = compiled(catalog, draft.disturbanceCommand);
  const real = (symbol: string) => realType(compiled(catalog, symbol));
  const record = draft.parameterRecord;
  const seed: FeedforwardPidSeed = { kind: "feedforward-pid", family: "PI", proportionalSymbol: `${record}.ProportionalGain`,
    integralSymbol: `${record}.IntegralGain`, derivativeSymbol: `${record}.DerivativeGain`, gainSymbol: gain,
    ...(lead && lag ? { leadSymbol: lead, lagSymbol: lag } : {}) };
  const parameters = [...pidParameters(record, pv, mv, ["ProportionalGain", "IntegralGain", "DerivativeGain"]),
    { symbol: gain, iec_type: real(gain), unit: `${mv.unit}/${disturbance.unit}`, convention: "Feedforward gain", minimum: -1e6, maximum: 1e6 },
    ...(lead && lag ? [{ symbol: lead, iec_type: real(lead), unit: "s", convention: "Feedforward lead time", minimum: 0, maximum: 1e4 },
      { symbol: lag, iec_type: real(lag), unit: "s", convention: "Feedforward lag time", minimum: 0.001, maximum: 1e4 }] : [])];
  // The controller's feedforward state starts at K_ff·d and the lag at d, so switching to Auto does not bump.
  const states = [...pidStates(draft.instance, pv, mv, 0).map(state => state.symbol.endsWith(".StateValue.FeedForward")
    ? { ...state, initial: { source: "scaled-model-input" as const, index: 1, gainSymbol: gain } } : state),
  ...(filtered ? [{ symbol: filtered, iec_type: real(filtered), unit: disturbance.unit, minimum: disturbance.minimum, maximum: disturbance.maximum,
    initial: { source: "model-input" as const, index: 1 }, postScan: "bounded" as const }] : []),
  ...(draft.startupFlag ? [startupState(draft.startupFlag)] : [])];
  return withRevision({
    schemaVersion: 2, id, name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
    topology: "closed-loop",
    controller: {
      instance: draft.instance, type: "Control.PID", design: "project", revision: "sha256:unverified", action: draft.action,
      taskPeriodMs: period, modeSignal: draft.mode.symbol, modeWhenTrue: draft.mode.whenTrue,
      manualOutputSignal: manual.symbol, manualOutputType: realType(manual), ...(manual.address ? { manualOutputAddress: manual.address } : {}),
      ...manualOutputTracking(catalog, draft.instance, manual.symbol),
      project: {
        family: "feedforward", convention: "project-owned-native-controller",
        initialization: { kind: "declared-state-v1", states }, seed,
        axes: [], objective: { requirementId: "target-error", direction: "minimize" },
        maximumCandidates: 1, maximumCycles: 50_000, wallBudgetMs: 60_000,
      },
    },
    signals: {
      setpoint: binding(catalog, setpoint, "read"),
      processVariable: measurementBinding(catalog, pv),
      manipulatedVariable: binding(catalog, mv, "write"),
      additional: [{ id: "measured-disturbance", channel: "measured-disturbance", role: "disturbance" as const, ...measurementBinding(catalog, disturbance) }],
    },
    channels: {
      inputs: [
        { modelChannelId: mv.symbol, signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: mv.minimum, maximum: mv.maximum },
          ...(draft.experimentStep === undefined ? {} : { experimentStep: draft.experimentStep }) },
        { modelChannelId: disturbance.symbol, signalId: "measured-disturbance", manualOutputSignal: command.symbol, manualOutputType: realType(command),
          actuator: { kind: "continuous", minimum: disturbance.minimum, maximum: disturbance.maximum }, experimentStep: draft.disturbanceStep },
      ],
      outputs: [{ modelChannelId: pv.symbol, signalId: "processVariable", setpointSignalId: "setpoint" }],
    },
    // Both paths' gains and time constants set the feedforward: a fit sharing one time constant must not pass (§17.12).
    identificationPolicy: { maximumOrder: 2, maximumNormalizedError: 0.05, openLoopPermitted: true, sustainedHoldPermitted: true, information: "basic" },
    limits: { outputMinimum: mv.minimum, outputMaximum: mv.maximum },
    observedSampleTiming: { nominalPeriodMs: period },
    ...explicitExperiment(draft.name, { execution: "control-studio", kind: "mv_step" }, draft.experimentStep, draft.experimentDurationSeconds),
    parameterBinding: {
      loop_id: id, task_name: task.name,
      mode: { symbol: draft.mode.symbol, manual_value: draft.mode.whenTrue === "Manual" },
      parameters,
    },
  } as ControlLoopDefinition);
}
