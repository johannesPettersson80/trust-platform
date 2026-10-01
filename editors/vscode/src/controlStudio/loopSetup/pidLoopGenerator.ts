import type { ControlLoopDefinition } from "../loopDefinition";
import { PID_TYPE, type CompiledCatalog } from "./compiledCatalog";
import { binding, measurementBinding, compiled, identificationDuration, loopId, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { pidParameters, pidStates, startupState } from "./pidBindings";

export interface SignalChoice { symbol: string; unit?: string; minimum?: number; maximum?: number }
export type { MeasuredSignal } from "./loopBindings";
export interface ModeChoice { symbol: string; whenTrue: "Auto" | "Manual" }

export interface PidLoopDraft {
  projectId: string; runtimeId: string; name: string;
  instance: string; action: "direct" | "reverse"; parameterRecord: string;
  processVariable: MeasuredSignal; manipulatedVariable: MeasuredSignal; setpoint: SignalChoice;
  mode: ModeChoice; manualOutput: string;
  /** The identification experiment's output step in the output unit; a tenth of the output range when absent. */
  experimentStep?: number;
  /** How long identification runs, in s. It depends on the process, so it is always the engineer's choice. */
  experimentDurationSeconds?: number;
  /** A BOOL the program sets when its startup code has run; validation starts with it TRUE so that code keeps the candidate. */
  startupFlag?: string;
}


/** A complete single-loop definition for a library PID, ready for Identify, Tune, Validate and Apply. */
export function pidLoopDefinition(catalog: CompiledCatalog, draft: PidLoopDraft): ControlLoopDefinition {
  return withRevision(pidLoop(catalog, draft, loopId(draft.name)));
}

/** A cascade as two linked loops: the outer output is the inner setpoint, in the inner measurement's unit and range. */

function pidLoop(catalog: CompiledCatalog, draft: PidLoopDraft, id: string): ControlLoopDefinition {
  const controller = catalog.controllers.find(entry => entry.instance === draft.instance && entry.type.toUpperCase() === PID_TYPE);
  if (!controller) throw new Error(`${draft.instance} is not a compiled Control.PID instance.`);
  if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${draft.instance} must run in exactly one cyclic task.`);
  const task = controller.tasks[0], period = task.intervalMs;
  const pv = draft.processVariable, mv = draft.manipulatedVariable;
  const setpoint: MeasuredSignal = { symbol: draft.setpoint.symbol, unit: draft.setpoint.unit ?? pv.unit,
    minimum: draft.setpoint.minimum ?? pv.minimum, maximum: draft.setpoint.maximum ?? pv.maximum };
  const manual = compiled(catalog, draft.manualOutput);
  const span = mv.maximum - mv.minimum;
  return {
    schemaVersion: 2, id, name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
    topology: "closed-loop",
    controller: {
      instance: draft.instance, type: "Control.PID", design: "pid", revision: "sha256:unverified", action: draft.action,
      taskPeriodMs: period, modeSignal: draft.mode.symbol, modeWhenTrue: draft.mode.whenTrue,
      manualOutputSignal: manual.symbol, manualOutputType: realType(manual), ...(manual.address ? { manualOutputAddress: manual.address } : {}),
      law: { kind: "pid", parameterRecordSymbol: draft.parameterRecord,
        proportionalGainSymbol: `${draft.parameterRecord}.ProportionalGain`, integralGainSymbol: `${draft.parameterRecord}.IntegralGain`,
        derivativeGainSymbol: `${draft.parameterRecord}.DerivativeGain` },
      pidInitialization: { kind: "control-pid-v1", maximumSnapshotAgeMs: Math.max(500, 5 * period),
        ...(draft.startupFlag ? { applicationStates: [startupState(draft.startupFlag)] } : {}),
        states: pidStates(draft.instance, pv, mv) },
    },
    signals: {
      setpoint: binding(catalog, setpoint, "read"),
      processVariable: measurementBinding(catalog, pv),
      manipulatedVariable: binding(catalog, mv, "write"),
    },
    limits: { outputMinimum: mv.minimum, outputMaximum: mv.maximum },
    observedSampleTiming: { nominalPeriodMs: period },
    experiment: { execution: "control-studio", kind: "mv_step", amplitude: draft.experimentStep ?? span / 10,
      durationSeconds: identificationDuration(draft.name, draft.experimentDurationSeconds), seed: 1 },
    parameterBinding: {
      loop_id: id, task_name: task.name,
      mode: { symbol: draft.mode.symbol, manual_value: draft.mode.whenTrue === "Manual" },
      parameters: pidParameters(draft.parameterRecord, pv, mv),
    },
  } as ControlLoopDefinition;
}
