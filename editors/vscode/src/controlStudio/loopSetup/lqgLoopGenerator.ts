import type { GeneralModel } from "../generalModel";
import type { ControlLoopDefinition } from "../loopDefinition";
import { ratioUnit, sampleModel } from "../stateControllerDesign";
import type { CompiledCatalog } from "./compiledCatalog";
import { binding, measurementBinding, compiled, explicitExperiment, loopId, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { startupState } from "./pidBindings";
import type { ModeChoice, SignalChoice } from "./pidLoopGenerator";

/** The program's estimator: one estimate, gain and observer gain per model state, and the noise variances (Specification 35 §17.1). */
export interface LqgEstimatorDraft {
  /** The project's model file the estimator implements; the save step reads it into `model`. */
  modelFile: string;
  model?: GeneralModel;
  states: readonly { estimate: string; gain: string; observerGain: string; processNoise: number }[];
  measurementNoise: number;
}

export interface LqgLoopDraft {
  projectId: string; runtimeId: string; name: string; action: "direct" | "reverse";
  instance: string;
  manipulatedVariable: MeasuredSignal; processVariable: MeasuredSignal; setpoint: SignalChoice;
  referenceGain: string;
  estimator: LqgEstimatorDraft;
  mode: ModeChoice; manualOutput: string;
  experimentStep?: number; experimentDurationSeconds?: number; startupFlag?: string;
}

/** An LQG law in the program: estimates from the controlled output, fed back with K and N (§17.8 G). */
export function lqgLoopDefinition(catalog: CompiledCatalog, draft: LqgLoopDraft): ControlLoopDefinition {
  const controller = catalog.controllers.find(entry => entry.instance === draft.instance);
  if (!controller) throw new Error(`${draft.instance} is not a compiled program or function block instance.`);
  if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${draft.instance} must run in exactly one cyclic task.`);
  const model = draft.estimator.model;
  if (!model || model.kind === "transfer-function" || model.inputs.length !== 1 || model.outputs.length !== 1) {
    throw new Error("The estimator's model must be a state-space model with one input and the controlled output as its one output.");
  }
  const estimates = draft.estimator.states;
  if (estimates.length !== model.states.length || estimates.length > 3) throw new Error("Give one estimate, gain and observer gain for each of the model's (at most three) states.");
  const positive = (value: number) => Number.isFinite(value) && value > 0;
  if (!estimates.every(state => positive(state.processNoise)) || !positive(draft.estimator.measurementNoise)) throw new Error("Noise variances must be positive.");
  const task = controller.tasks[0], period = task.intervalMs, id = loopId(draft.name);
  const sampled = sampleModel(model, period / 1000);
  const pv = draft.processVariable, mv = draft.manipulatedVariable, span = mv.maximum - mv.minimum;
  const setpoint: MeasuredSignal = { symbol: draft.setpoint.symbol, unit: draft.setpoint.unit ?? pv.unit,
    minimum: draft.setpoint.minimum ?? pv.minimum, maximum: draft.setpoint.maximum ?? pv.maximum };
  const manual = compiled(catalog, draft.manualOutput), real = (symbol: string) => realType(compiled(catalog, symbol));
  const stateSpan = (index: number) => model.states[index].operatingRange.maximum - model.states[index].operatingRange.minimum;
  const coefficient = (symbol: string, unit: string, limit: number, convention: string) =>
    ({ symbol, iec_type: real(symbol), unit, convention, minimum: -limit, maximum: limit });
  return withRevision({
    schemaVersion: 2, id, name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
    topology: "closed-loop",
    controller: {
      instance: draft.instance, type: controller.type, design: "project", revision: "sha256:unverified", action: draft.action,
      taskPeriodMs: period, modeSignal: draft.mode.symbol, modeWhenTrue: draft.mode.whenTrue,
      manualOutputSignal: manual.symbol, manualOutputType: realType(manual), ...(manual.address ? { manualOutputAddress: manual.address } : {}),
      stateFeedback: {
        family: "LQG", convention: "sampled-deviation-negative-feedback",
        gainSymbols: [estimates.map(state => state.gain)], referenceGainSymbols: [[draft.referenceGain]],
        observerGainSymbols: estimates.map(state => [state.observerGain]),
        estimatorModel: { a: sampled.a, b: sampled.b, c: sampled.c, sampleTimeSeconds: period / 1000 },
        processNoise: estimates.map((state, row) => estimates.map((_, column) => row === column ? state.processNoise : 0)),
        measurementNoise: [[draft.estimator.measurementNoise]],
        // The estimates are deviations from the model's operating point, where they start (§17.1).
        initialization: { kind: "declared-state-v1", states: [...estimates.map((state, index) => ({ symbol: state.estimate, iec_type: real(state.estimate),
          unit: model.states[index].unit, minimum: model.states[index].operatingRange.minimum, maximum: model.states[index].operatingRange.maximum,
          initial: 0, postScan: "bounded" as const })), ...(draft.startupFlag ? [startupState(draft.startupFlag)] : [])] },
        estimatorStateSymbols: estimates.map(state => state.estimate),
      },
    },
    signals: {
      setpoint: binding(catalog, setpoint, "read"),
      processVariable: measurementBinding(catalog, pv),
      manipulatedVariable: binding(catalog, mv, "write"),
    },
    // The supplied model maps by its own channel names.
    channels: {
      inputs: [{ modelChannelId: model.inputs[0].id, signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: mv.minimum, maximum: mv.maximum } }],
      outputs: [{ modelChannelId: model.outputs[0].id, signalId: "processVariable", setpointSignalId: "setpoint" }],
    },
    identificationPolicy: { maximumOrder: estimates.length, openLoopPermitted: true, sustainedHoldPermitted: true, information: "basic" },
    limits: { outputMinimum: mv.minimum, outputMaximum: mv.maximum },
    observedSampleTiming: { nominalPeriodMs: period },
    ...explicitExperiment(draft.name, { execution: "control-studio", kind: "mv_step" }, draft.experimentStep, draft.experimentDurationSeconds),
    parameterBinding: {
      loop_id: id, task_name: task.name, mode: { symbol: draft.mode.symbol, manual_value: draft.mode.whenTrue === "Manual" },
      parameters: [
        // A coefficient larger than these would move its target across its whole range for a thousandth of the source's range.
        ...estimates.map((state, index) => coefficient(state.gain, ratioUnit(mv.unit, model.states[index].unit), 1000 * span / stateSpan(index),
          `LQG gain on ${state.estimate}`)),
        coefficient(draft.referenceGain, ratioUnit(mv.unit, pv.unit), 1000 * span / (pv.maximum - pv.minimum), `Reference gain on ${setpoint.symbol}`),
        ...estimates.map((state, index) => coefficient(state.observerGain, ratioUnit(model.states[index].unit, pv.unit),
          1000 * stateSpan(index) / (pv.maximum - pv.minimum), `Observer gain on ${state.estimate}`)),
      ],
    },
  } as ControlLoopDefinition);
}
