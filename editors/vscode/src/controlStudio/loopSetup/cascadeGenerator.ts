import type { ControlLoopDefinition } from "../loopDefinition";
import { PID_TYPE, type CompiledCatalog } from "./compiledCatalog";
import { binding, measurementBinding, compiled, identificationDuration, loopId, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { pidParameters, pidStates, startupState } from "./pidBindings";
import type { ModeChoice, SignalChoice } from "./pidLoopGenerator";

/** A cascade tuned together: the outer `Control.PID`'s output is the inner one's setpoint (Specification 35 §17.11). */
export interface CascadeTogetherDraft {
  projectId: string; runtimeId: string; name: string; action: "direct" | "reverse";
  outer: { instance: string; parameterRecord: string; mode: ModeChoice; manualOutput: string; processVariable: MeasuredSignal; setpoint: SignalChoice };
  inner: { instance: string; parameterRecord: string; mode: ModeChoice; manualOutput: string; processVariable: MeasuredSignal; manipulatedVariable: MeasuredSignal };
  /** The plant input's identification step, in its unit; a tenth of its range when absent. */
  experimentStep?: number;
  /** One identification long enough for the outer measurement to settle. */
  experimentDurationSeconds?: number;
  startupFlag?: string;
}

/** One loop for both controllers: the plant input moves both measurements, and both controllers change mode together. */
export function cascadeLoopDefinition(catalog: CompiledCatalog, draft: CascadeTogetherDraft): ControlLoopDefinition {
  const task = (instance: string) => {
    const controller = catalog.controllers.find(entry => entry.instance === instance && entry.type.toUpperCase() === PID_TYPE);
    if (!controller) throw new Error(`${instance} is not a compiled Control.PID instance.`);
    if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${instance} must run in exactly one cyclic task.`);
    return controller.tasks[0];
  };
  const innerTask = task(draft.inner.instance), outerTask = task(draft.outer.instance);
  if (draft.inner.instance === draft.outer.instance) throw new Error("Choose two different controllers.");
  if (innerTask.name !== outerTask.name) throw new Error("Both controllers of a cascade must run in the same task.");
  if (draft.inner.mode.symbol === draft.outer.mode.symbol) throw new Error("Each controller of a cascade needs its own Auto/Manual signal.");
  const id = loopId(draft.name), period = innerTask.intervalMs;
  const mv = draft.inner.manipulatedVariable, inner = draft.inner.processVariable, outer = draft.outer.processVariable;
  const setpoint: MeasuredSignal = { symbol: draft.outer.setpoint.symbol, unit: draft.outer.setpoint.unit ?? outer.unit,
    minimum: draft.outer.setpoint.minimum ?? outer.minimum, maximum: draft.outer.setpoint.maximum ?? outer.maximum };
  const manual = compiled(catalog, draft.inner.manualOutput), span = mv.maximum - mv.minimum;
  const gains = ["ProportionalGain", "IntegralGain", "DerivativeGain"] as const;
  const symbols = (instance: string, record: string) => ({ instance, proportionalSymbol: `${record}.ProportionalGain`,
    integralSymbol: `${record}.IntegralGain`, derivativeSymbol: `${record}.DerivativeGain` });
  // The outer controller's output is the inner setpoint: it starts at the inner measurement, so neither switch bumps.
  const outerStates = pidStates(draft.outer.instance, outer, inner, 1).map(state => state.initial !== null && typeof state.initial === "object"
    && state.initial.source === "model-input" ? { ...state, initial: { source: "model-output" as const, index: 0 } } : state);
  return withRevision({
    schemaVersion: 2, id, name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
    topology: "closed-loop",
    controller: {
      instance: draft.inner.instance, type: "Control.PID", design: "project", revision: "sha256:unverified", action: draft.action,
      taskPeriodMs: period, modeSignal: draft.inner.mode.symbol, modeWhenTrue: draft.inner.mode.whenTrue,
      manualOutputSignal: manual.symbol, manualOutputType: realType(manual), ...(manual.address ? { manualOutputAddress: manual.address } : {}),
      project: {
        family: "cascade", convention: "project-owned-native-controller",
        additionalModes: [{ symbol: draft.outer.mode.symbol, manualValue: draft.outer.mode.whenTrue === "Manual" }],
        initialization: { kind: "declared-state-v1", states: [...pidStates(draft.inner.instance, inner, mv, 0), ...outerStates,
          ...(draft.startupFlag ? [startupState(draft.startupFlag)] : [])] },
        seed: { kind: "cascade-pid", family: "PI", inner: symbols(draft.inner.instance, draft.inner.parameterRecord),
          outer: symbols(draft.outer.instance, draft.outer.parameterRecord) },
        axes: [], objective: { requirementId: `${outer.symbol}:target-error`, direction: "minimize" },
        maximumCandidates: 1, maximumCycles: 50_000, wallBudgetMs: 60_000,
      },
    },
    signals: {
      setpoint: binding(catalog, setpoint, "read"),
      processVariable: measurementBinding(catalog, outer),
      manipulatedVariable: binding(catalog, mv, "write"),
      additional: [{ id: "inner-measurement", channel: "inner-measurement", role: "state" as const, ...measurementBinding(catalog, inner) }],
    },
    channels: {
      inputs: [{ modelChannelId: mv.symbol, signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: mv.minimum, maximum: mv.maximum } }],
      outputs: [{ modelChannelId: inner.symbol, signalId: "inner-measurement" },
        { modelChannelId: outer.symbol, signalId: "processVariable", setpointSignalId: "setpoint" }],
    },
    identificationPolicy: { maximumOrder: 1, openLoopPermitted: true, sustainedHoldPermitted: true, information: "basic" },
    limits: { outputMinimum: mv.minimum, outputMaximum: mv.maximum },
    observedSampleTiming: { nominalPeriodMs: period },
    experiment: { execution: "control-studio", kind: "mv_step", amplitude: draft.experimentStep ?? span / 10,
      durationSeconds: identificationDuration(draft.name, draft.experimentDurationSeconds), seed: 1 },
    parameterBinding: {
      loop_id: id, task_name: innerTask.name,
      mode: { symbol: draft.inner.mode.symbol, manual_value: draft.inner.mode.whenTrue === "Manual" },
      parameters: [...pidParameters(draft.inner.parameterRecord, inner, mv, gains), ...pidParameters(draft.outer.parameterRecord, outer, inner, gains)],
    },
  } as ControlLoopDefinition);
}
