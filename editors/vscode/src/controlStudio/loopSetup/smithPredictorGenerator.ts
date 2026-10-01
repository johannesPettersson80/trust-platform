import type { ControlLoopDefinition } from "../loopDefinition";
import type { SmithPidSeed } from "../smithPredictorDesign";
import { PID_TYPE, type CompiledCatalog } from "./compiledCatalog";
import { binding, compiled, identificationDuration, loopId, measurementBinding, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { pidParameters, pidStates, startupState } from "./pidBindings";
import type { ModeChoice, SignalChoice } from "./pidLoopGenerator";

/** A `Control.PID` fed the measurement plus a Smith predictor's correction the program computes (Specification 35 §17.13). */
export interface SmithPredictorDraft {
  projectId: string; runtimeId: string; name: string;
  instance: string; action: "direct" | "reverse"; parameterRecord: string;
  /** The process measurement, which identification and the requirements use, not the corrected value the PID reads. */
  processVariable: MeasuredSignal; manipulatedVariable: MeasuredSignal; setpoint: SignalChoice;
  mode: ModeChoice; manualOutput: string;
  /** The program's model gain, time constant and delay (in s) variables and the correction it keeps. */
  model: { gain: string; timeConstant: string; delay: string; correction: string };
  experimentStep?: number;
  experimentDurationSeconds?: number;
  startupFlag?: string;
}

export function smithPredictorLoopDefinition(catalog: CompiledCatalog, draft: SmithPredictorDraft): ControlLoopDefinition {
  const controller = catalog.controllers.find(entry => entry.instance === draft.instance && entry.type.toUpperCase() === PID_TYPE);
  if (!controller) throw new Error(`${draft.instance} is not a compiled Control.PID instance.`);
  if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${draft.instance} must run in exactly one cyclic task.`);
  const task = controller.tasks[0], period = task.intervalMs, id = loopId(draft.name);
  const pv = draft.processVariable, mv = draft.manipulatedVariable, { gain, timeConstant, delay, correction } = draft.model;
  const owned = [gain, timeConstant, delay, correction];
  if (new Set([...owned, pv.symbol, mv.symbol].map(symbol => symbol.toUpperCase())).size !== owned.length + 2) {
    throw new Error("The predictor's model variables and correction must differ from each other and from the loop's signals.");
  }
  const setpoint: MeasuredSignal = { symbol: draft.setpoint.symbol, unit: draft.setpoint.unit ?? pv.unit,
    minimum: draft.setpoint.minimum ?? pv.minimum, maximum: draft.setpoint.maximum ?? pv.maximum };
  const manual = compiled(catalog, draft.manualOutput), real = (symbol: string) => realType(compiled(catalog, symbol));
  const record = draft.parameterRecord, span = pv.maximum - pv.minimum;
  const seed: SmithPidSeed = { kind: "smith-pid", family: "PI", proportionalSymbol: `${record}.ProportionalGain`,
    integralSymbol: `${record}.IntegralGain`, derivativeSymbol: `${record}.DerivativeGain`,
    gainSymbol: gain, timeConstantSymbol: timeConstant, delaySymbol: delay };
  const parameters = [...pidParameters(record, pv, mv, ["ProportionalGain", "IntegralGain", "DerivativeGain"]),
    { symbol: gain, iec_type: real(gain), unit: `${pv.unit}/${mv.unit}`, convention: "Smith predictor model gain", minimum: -1e6, maximum: 1e6 },
    { symbol: timeConstant, iec_type: real(timeConstant), unit: "s", convention: "Smith predictor model time constant", minimum: 0.001, maximum: 1e5 },
    { symbol: delay, iec_type: real(delay), unit: "s", convention: "Smith predictor model delay", minimum: 0, maximum: 1e4 }];
  // The correction is zero whenever the output has been steady, so the switch to Auto starts it at zero (§17.13).
  const states = [...pidStates(draft.instance, pv, mv, 0),
    { symbol: correction, iec_type: real(correction), unit: pv.unit, minimum: -span, maximum: span, initial: 0, postScan: "bounded" as const },
    ...(draft.startupFlag ? [startupState(draft.startupFlag)] : [])];
  return withRevision({
    schemaVersion: 2, id, name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
    topology: "closed-loop",
    controller: {
      instance: draft.instance, type: "Control.PID", design: "project", revision: "sha256:unverified", action: draft.action,
      taskPeriodMs: period, modeSignal: draft.mode.symbol, modeWhenTrue: draft.mode.whenTrue,
      manualOutputSignal: manual.symbol, manualOutputType: realType(manual), ...(manual.address ? { manualOutputAddress: manual.address } : {}),
      project: {
        family: "Smith", convention: "project-owned-native-controller",
        initialization: { kind: "declared-state-v1", states }, seed,
        axes: [], objective: { requirementId: "target-error", direction: "minimize" },
        maximumCandidates: 1, maximumCycles: 50_000, wallBudgetMs: 60_000,
      },
    },
    signals: {
      setpoint: binding(catalog, setpoint, "read"),
      processVariable: measurementBinding(catalog, pv),
      manipulatedVariable: binding(catalog, mv, "write"),
    },
    limits: { outputMinimum: mv.minimum, outputMaximum: mv.maximum },
    observedSampleTiming: { nominalPeriodMs: period },
    experiment: { execution: "control-studio", kind: "mv_step", amplitude: draft.experimentStep ?? (mv.maximum - mv.minimum) / 10,
      durationSeconds: identificationDuration(draft.name, draft.experimentDurationSeconds), seed: 1 },
    parameterBinding: {
      loop_id: id, task_name: task.name,
      mode: { symbol: draft.mode.symbol, manual_value: draft.mode.whenTrue === "Manual" },
      parameters,
    },
  } as ControlLoopDefinition);
}
