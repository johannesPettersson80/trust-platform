import type { ControlLoopDefinition } from "../loopDefinition";
import { PID_TYPE, type CompiledCatalog } from "./compiledCatalog";
import { binding, compiled, identificationDuration, loopId, measurementBinding, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { pidStates, startupState } from "./pidBindings";
import type { ModeChoice, SignalChoice } from "./pidLoopGenerator";

/** A `Control.PID` whose program interpolates its gains from a schedule table over operating points (Specification 35 §17.14). */
export interface GainScheduleDraft {
  projectId: string; runtimeId: string; name: string;
  instance: string; action: "direct" | "reverse";
  processVariable: MeasuredSignal; manipulatedVariable: MeasuredSignal; setpoint: SignalChoice;
  mode: ModeChoice; manualOutput: string;
  /** Two to four operating points, each with the program's proportional and integral gain variables for it. */
  points: readonly { name: string; proportionalGain: string; integralGain: string }[];
  experimentStep?: number; experimentDurationSeconds?: number; startupFlag?: string;
}

/** One loop per operating point: identified, tuned, validated and applied there, with the program's schedule active. */
export function gainScheduleLoopDefinitions(catalog: CompiledCatalog, draft: GainScheduleDraft): ControlLoopDefinition[] {
  const controller = catalog.controllers.find(entry => entry.instance === draft.instance && entry.type.toUpperCase() === PID_TYPE);
  if (!controller) throw new Error(`${draft.instance} is not a compiled Control.PID instance.`);
  if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${draft.instance} must run in exactly one cyclic task.`);
  if (draft.points.length < 2 || draft.points.length > 4) throw new Error("A gain schedule needs two to four operating points.");
  const names = draft.points.map(point => point.name.trim().toLowerCase()), symbols = draft.points.flatMap(point => [point.proportionalGain, point.integralGain]);
  if (names.some(name => !name) || new Set(names).size !== names.length || new Set(symbols.map(symbol => symbol.toUpperCase())).size !== symbols.length) {
    throw new Error("Give each operating point its own name and its own two gain variables.");
  }
  const task = controller.tasks[0], period = task.intervalMs;
  const pv = draft.processVariable, mv = draft.manipulatedVariable, span = mv.maximum - mv.minimum;
  const setpoint: MeasuredSignal = { symbol: draft.setpoint.symbol, unit: draft.setpoint.unit ?? pv.unit,
    minimum: draft.setpoint.minimum ?? pv.minimum, maximum: draft.setpoint.maximum ?? pv.maximum };
  const manual = compiled(catalog, draft.manualOutput);
  const gain = (symbol: string, unit: string, convention: string) => ({ symbol, iec_type: realType(compiled(catalog, symbol)), unit, convention, minimum: 0, maximum: 1e6 });
  return draft.points.map(point => {
    const name = `${draft.name} · ${point.name.trim()}`, id = loopId(`${draft.name} ${point.name}`);
    return withRevision({
      schemaVersion: 2, id, name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
      topology: "closed-loop",
      controller: {
        instance: draft.instance, type: "Control.PID", design: "project", revision: "sha256:unverified", action: draft.action,
        taskPeriodMs: period, modeSignal: draft.mode.symbol, modeWhenTrue: draft.mode.whenTrue,
        manualOutputSignal: manual.symbol, manualOutputType: realType(manual), ...(manual.address ? { manualOutputAddress: manual.address } : {}),
        project: {
          family: "gain-scheduled", convention: "project-owned-native-controller",
          initialization: { kind: "declared-state-v1", states: [...pidStates(draft.instance, pv, mv, 0), ...(draft.startupFlag ? [startupState(draft.startupFlag)] : [])] },
          seed: { kind: "parallel-pid", family: "PI", proportionalSymbol: point.proportionalGain, integralSymbol: point.integralGain },
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
      experiment: { execution: "control-studio", kind: "mv_step", amplitude: draft.experimentStep ?? span / 10,
        durationSeconds: identificationDuration(name, draft.experimentDurationSeconds), seed: 1 },
      parameterBinding: {
        loop_id: id, task_name: task.name, mode: { symbol: draft.mode.symbol, manual_value: draft.mode.whenTrue === "Manual" },
        parameters: [gain(point.proportionalGain, `${mv.unit}/${pv.unit}`, `Scheduled proportional gain at ${point.name.trim()}`),
          gain(point.integralGain, `${mv.unit}/(${pv.unit}*s)`, `Scheduled integral gain at ${point.name.trim()}`)],
      },
    } as ControlLoopDefinition);
  });
}
