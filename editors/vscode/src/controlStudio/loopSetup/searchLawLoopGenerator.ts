import type { ControlLoopDefinition } from "../loopDefinition";
import type { CompiledCatalog } from "./compiledCatalog";
import { binding, measurementBinding, compiled, identificationDuration, loopId, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { startupState } from "./pidBindings";
import type { ModeChoice, SignalChoice } from "./pidLoopGenerator";

/** Laws whose coefficients no design formula in Studio sets; Tune searches them (Specification 35 §17.1). */
export const SEARCH_FAMILIES = ["MPC", "ratio", "split-range", "override", "gain-scheduled", "feedforward", "Smith", "lead-lag",
  "adaptive", "robust", "feedback-linearization", "observer-feedback"] as const;
export type SearchFamily = typeof SEARCH_FAMILIES[number];

export interface SearchCoefficient { symbol: string; unit: string; values: readonly number[] }
/** A value the program keeps from scan to scan, and what validation starts it at. */
export interface RetainedValue { symbol: string; unit: string; start: "zero" | "output" | "measurement" }

export interface SearchLawLoopDraft {
  projectId: string; runtimeId: string; name: string; action: "direct" | "reverse";
  family: SearchFamily;
  /** The compiled program or function block instance that computes the law. */
  instance: string;
  processVariable: MeasuredSignal; manipulatedVariable: MeasuredSignal; setpoint: SignalChoice;
  mode: ModeChoice; manualOutput: string;
  coefficients: readonly SearchCoefficient[];
  states: readonly RetainedValue[];
  experimentStep?: number; experimentDurationSeconds?: number; startupFlag?: string;
}

/** A project-owned controller whose declared coefficient values Tune tries in every combination on the actual ST. */
export function searchLawLoopDefinition(catalog: CompiledCatalog, draft: SearchLawLoopDraft): ControlLoopDefinition {
  const controller = catalog.controllers.find(entry => entry.instance === draft.instance);
  if (!controller) throw new Error(`${draft.instance} is not a compiled program or function block instance.`);
  if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${draft.instance} must run in exactly one cyclic task.`);
  if (!(SEARCH_FAMILIES as readonly string[]).includes(draft.family)) throw new Error("Choose which kind of law the program computes.");
  if (!draft.coefficients.length) throw new Error("Choose at least one coefficient Tune may search.");
  if (draft.coefficients.length > 4) throw new Error("Tune searches at most four coefficients.");
  if (draft.states.length > 3) throw new Error("Declare at most three values the program keeps from scan to scan.");
  const variables = [...draft.coefficients.map(coefficient => coefficient.symbol), ...draft.states.map(state => state.symbol)];
  if (new Set(variables.map(symbol => symbol.toUpperCase())).size !== variables.length) {
    throw new Error("Choose a different variable for each coefficient and each value the program keeps.");
  }
  const axes = draft.coefficients.map(coefficient => {
    if (!coefficient.values.length || coefficient.values.some(value => !Number.isFinite(value))) {
      throw new Error(`Enter the values of ${coefficient.symbol} Tune may try.`);
    }
    const type = realType(compiled(catalog, coefficient.symbol));
    // A REAL holds the nearest single-precision value, so that is the value Tune tries.
    const values = [...new Set(coefficient.values.map(value => type === "REAL" ? Math.fround(value) : value))];
    return { symbol: coefficient.symbol, type, unit: coefficient.unit, values };
  });
  const combinations = axes.reduce((count, axis) => count * axis.values.length, 1);
  if (combinations > 64) throw new Error(`Tune tries at most 64 combinations; these values make ${combinations}.`);
  const task = controller.tasks[0], period = task.intervalMs, id = loopId(draft.name);
  const pv = draft.processVariable, mv = draft.manipulatedVariable, span = mv.maximum - mv.minimum, pvSpan = pv.maximum - pv.minimum;
  const setpoint: MeasuredSignal = { symbol: draft.setpoint.symbol, unit: draft.setpoint.unit ?? pv.unit,
    minimum: draft.setpoint.minimum ?? pv.minimum, maximum: draft.setpoint.maximum ?? pv.maximum };
  const manual = compiled(catalog, draft.manualOutput);
  const retained = draft.states.map(state => ({ symbol: state.symbol, iec_type: realType(compiled(catalog, state.symbol)), unit: state.unit,
    ...(state.start === "output" ? { minimum: mv.minimum - 10 * span, maximum: mv.maximum + 10 * span, initial: { source: "model-input" as const, index: 0 } }
      : state.start === "measurement" ? { minimum: pv.minimum - pvSpan, maximum: pv.maximum + pvSpan, initial: { source: "model-output" as const, index: 0 } }
      : { minimum: -1e9, maximum: 1e9, initial: 0 }), postScan: "bounded" as const }));
  return withRevision({
    schemaVersion: 2, id, name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
    topology: "closed-loop",
    controller: {
      instance: draft.instance, type: controller.type, design: "project", revision: "sha256:unverified", action: draft.action,
      taskPeriodMs: period, modeSignal: draft.mode.symbol, modeWhenTrue: draft.mode.whenTrue,
      manualOutputSignal: manual.symbol, manualOutputType: realType(manual), ...(manual.address ? { manualOutputAddress: manual.address } : {}),
      project: {
        family: draft.family, convention: "project-owned-native-controller",
        initialization: { kind: "declared-state-v1", states: [...retained, ...(draft.startupFlag ? [startupState(draft.startupFlag)] : [])] },
        axes: axes.map(axis => ({ symbol: axis.symbol, values: axis.values })),
        // One controlled output: its verification campaign names the error requirement without a channel prefix.
        objective: { requirementId: "target-error", direction: "minimize" },
        maximumCandidates: 64, maximumCycles: 50_000, wallBudgetMs: 60_000,
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
      durationSeconds: identificationDuration(draft.name, draft.experimentDurationSeconds), seed: 1 },
    parameterBinding: {
      loop_id: id, task_name: task.name, mode: { symbol: draft.mode.symbol, manual_value: draft.mode.whenTrue === "Manual" },
      // Each coefficient is shown by its own variable name.
      parameters: axes.map(axis => ({ symbol: axis.symbol, iec_type: axis.type, unit: axis.unit, convention: axis.symbol.split(".").pop()!,
        minimum: Math.min(...axis.values), maximum: Math.max(...axis.values) })),
    },
  } as ControlLoopDefinition);
}
