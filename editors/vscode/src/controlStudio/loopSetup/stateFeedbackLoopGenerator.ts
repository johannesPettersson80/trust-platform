import type { ControlLoopDefinition } from "../loopDefinition";
import { ratioUnit } from "../stateControllerDesign";
import type { CompiledCatalog } from "./compiledCatalog";
import { binding, measurementBinding, compiled, identificationDuration, loopId, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { startupState } from "./pidBindings";
import { lqgLoopDefinition, type LqgLoopDraft } from "./lqgLoopGenerator";
import type { ModeChoice, SignalChoice } from "./pidLoopGenerator";

/** An existing state-feedback law in the program, u = u₀ + N·(r − y₀) − Σ Kᵢ·(yᵢ − yᵢ₀) (Specification 35 §17.1). */
export interface StateFeedbackLoopDraft {
  projectId: string; runtimeId: string; name: string; action: "direct" | "reverse";
  family: "state-feedback" | "LQR";
  /** The compiled program or function block instance that runs the law. */
  instance: string;
  manipulatedVariable: MeasuredSignal;
  processVariable: MeasuredSignal; setpoint: SignalChoice;
  /** The variables holding the controlled output's gain and the reference gain. */
  gain: string; referenceGain: string;
  /** Further measured outputs; each is a model state with the variable holding its gain. */
  states: readonly { signal: MeasuredSignal; gain: string }[];
  mode: ModeChoice; manualOutput: string;
  experimentStep?: number; experimentDurationSeconds?: number; startupFlag?: string;
}

/** A state-feedback loop identified in its measured coordinates, so that every gain belongs to one measured output; LQG
 * estimates the states instead (Specification 35 §17.1). */
export function stateFeedbackLoopDefinition(catalog: CompiledCatalog, draft: StateFeedbackLoopDraft | (LqgLoopDraft & { family: "LQG" })): ControlLoopDefinition {
  if (draft.family === "LQG") return lqgLoopDefinition(catalog, draft);
  const controller = catalog.controllers.find(entry => entry.instance === draft.instance);
  if (!controller) throw new Error(`${draft.instance} is not a compiled program or function block instance.`);
  if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${draft.instance} must run in exactly one cyclic task.`);
  if (draft.states.length < 1 || draft.states.length > 3) throw new Error("State feedback needs one to three measured outputs besides the controlled output.");
  const task = controller.tasks[0], period = task.intervalMs, id = loopId(draft.name);
  const pv = draft.processVariable, mv = draft.manipulatedVariable, span = mv.maximum - mv.minimum;
  const setpoint: MeasuredSignal = { symbol: draft.setpoint.symbol, unit: draft.setpoint.unit ?? pv.unit,
    minimum: draft.setpoint.minimum ?? pv.minimum, maximum: draft.setpoint.maximum ?? pv.maximum };
  const manual = compiled(catalog, draft.manualOutput);
  const measured = [{ signal: pv, gain: draft.gain }, ...draft.states];
  // A coefficient larger than this would move the input across its whole range for a thousandth of the output's range.
  const coefficient = (symbol: string, output: MeasuredSignal, convention: string) => {
    const limit = 1000 * span / (output.maximum - output.minimum);
    return { symbol, iec_type: realType(compiled(catalog, symbol)), unit: ratioUnit(mv.unit, output.unit), convention, minimum: -limit, maximum: limit };
  };
  return withRevision({
    schemaVersion: 2, id, name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
    topology: "closed-loop",
    controller: {
      instance: draft.instance, type: controller.type, design: "project", revision: "sha256:unverified", action: draft.action,
      taskPeriodMs: period, modeSignal: draft.mode.symbol, modeWhenTrue: draft.mode.whenTrue,
      manualOutputSignal: manual.symbol, manualOutputType: realType(manual), ...(manual.address ? { manualOutputAddress: manual.address } : {}),
      stateFeedback: {
        family: draft.family, convention: "sampled-deviation-negative-feedback",
        gainSymbols: [measured.map(state => state.gain)], referenceGainSymbols: [[draft.referenceGain]],
        initialization: { kind: "declared-state-v1", states: draft.startupFlag ? [startupState(draft.startupFlag)] : [] },
      },
    },
    signals: {
      setpoint: binding(catalog, setpoint, "read"),
      processVariable: measurementBinding(catalog, pv),
      manipulatedVariable: binding(catalog, mv, "write"),
      additional: draft.states.map((state, index) => ({ id: `state-${index + 2}`, channel: `state-${index + 2}`, role: "state" as const,
        ...measurementBinding(catalog, state.signal) })),
    },
    // The model's states are the measured outputs, in this order; the gain row follows it.
    channels: {
      inputs: [{ modelChannelId: mv.symbol, signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: mv.minimum, maximum: mv.maximum } }],
      outputs: [{ modelChannelId: pv.symbol, signalId: "processVariable", setpointSignalId: "setpoint" },
        ...draft.states.map((state, index) => ({ modelChannelId: state.signal.symbol, signalId: `state-${index + 2}` }))],
    },
    identificationPolicy: { maximumOrder: 1, openLoopPermitted: true, sustainedHoldPermitted: true, information: "basic" },
    limits: { outputMinimum: mv.minimum, outputMaximum: mv.maximum },
    observedSampleTiming: { nominalPeriodMs: period },
    experiment: { execution: "control-studio", kind: "mv_step", amplitude: draft.experimentStep ?? span / 10,
      durationSeconds: identificationDuration(draft.name, draft.experimentDurationSeconds), seed: 1 },
    parameterBinding: {
      loop_id: id, task_name: task.name, mode: { symbol: draft.mode.symbol, manual_value: draft.mode.whenTrue === "Manual" },
      parameters: [
        ...measured.map(state => coefficient(state.gain, state.signal, `State feedback gain on ${state.signal.symbol}`)),
        coefficient(draft.referenceGain, pv, `Reference gain on ${setpoint.symbol}`),
      ],
    },
  } as ControlLoopDefinition);
}
