import type { ControlLoopDefinition } from "../loopDefinition";
import type { CompiledCatalog } from "./compiledCatalog";
import { binding, measurementBinding, compiled, identificationDuration, loopId, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { startupState } from "./pidBindings";
import type { ModeChoice, SignalChoice } from "./pidLoopGenerator";

/** A variable holding one gain of the program's own law, with the range Tune may choose from. */
export interface OwnLawGain { symbol: string; minimum: number; maximum: number }

/** A P, PI or PID law the program computes itself, u = Kp·e + I + D with I = u₀ + Ki·∫e dt (Specification 35 §17.1). */
export interface OwnLawLoopDraft {
  projectId: string; runtimeId: string; name: string; action: "direct" | "reverse";
  law: "P" | "PI" | "PID";
  /** The compiled program or function block instance that computes the law. */
  instance: string;
  processVariable: MeasuredSignal; manipulatedVariable: MeasuredSignal; setpoint: SignalChoice;
  mode: ModeChoice; manualOutput: string;
  proportionalGain: OwnLawGain;
  integralGain?: OwnLawGain;
  /** The variable holding the integral term I, in the output's unit. */
  integralTerm?: string;
  derivativeGain?: OwnLawGain;
  /** The variable in which the program keeps the previous error or measurement its derivative is taken from. */
  derivativeState?: string;
  derivativeOn?: "error" | "measurement";
  experimentStep?: number; experimentDurationSeconds?: number; startupFlag?: string;
}

/** A project-owned controller whose seed names the program's gain variables, so Tune designs them from the model. */
export function ownLawLoopDefinition(catalog: CompiledCatalog, draft: OwnLawLoopDraft): ControlLoopDefinition {
  const controller = catalog.controllers.find(entry => entry.instance === draft.instance);
  if (!controller) throw new Error(`${draft.instance} is not a compiled program or function block instance.`);
  if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${draft.instance} must run in exactly one cyclic task.`);
  const integrating = draft.law !== "P", differentiating = draft.law === "PID";
  if (integrating && !draft.integralGain) throw new Error(`A ${draft.law} law needs the variable holding its integral gain.`);
  if (integrating && !draft.integralTerm) throw new Error(`A ${draft.law} law needs the variable holding its integral term.`);
  if (differentiating && !draft.derivativeGain) throw new Error("A PID law needs the variable holding its derivative gain.");
  if (differentiating && (!draft.derivativeState || !draft.derivativeOn)) {
    throw new Error("A PID law needs the variable holding the previous error or measurement its derivative is taken from.");
  }
  const variables = [draft.proportionalGain.symbol, ...(integrating ? [draft.integralGain!.symbol, draft.integralTerm!] : []),
    ...(differentiating ? [draft.derivativeGain!.symbol, draft.derivativeState!] : [])];
  if (new Set(variables.map(symbol => symbol.toUpperCase())).size !== variables.length) {
    throw new Error("Choose a different variable for each gain and for the integral term.");
  }
  for (const value of [draft.proportionalGain, ...(integrating ? [draft.integralGain!] : []), ...(differentiating ? [draft.derivativeGain!] : [])]) {
    if (!Number.isFinite(value.minimum) || !Number.isFinite(value.maximum) || value.minimum >= value.maximum) {
      throw new Error(`The range of ${value.symbol} needs a minimum below its maximum.`);
    }
  }
  const task = controller.tasks[0], period = task.intervalMs, id = loopId(draft.name);
  const pv = draft.processVariable, mv = draft.manipulatedVariable, span = mv.maximum - mv.minimum;
  const setpoint: MeasuredSignal = { symbol: draft.setpoint.symbol, unit: draft.setpoint.unit ?? pv.unit,
    minimum: draft.setpoint.minimum ?? pv.minimum, maximum: draft.setpoint.maximum ?? pv.maximum };
  const manual = compiled(catalog, draft.manualOutput);
  const integral = integrating ? { gain: draft.integralGain!, term: draft.integralTerm! } : undefined;
  const derivative = differentiating ? { gain: draft.derivativeGain!, state: draft.derivativeState!, on: draft.derivativeOn! } : undefined;
  const gain = (value: OwnLawGain, unit: string, convention: string) => ({ symbol: value.symbol, iec_type: realType(compiled(catalog, value.symbol)),
    unit, convention, minimum: value.minimum, maximum: value.maximum });
  // The integral term starts at the output of the switch to Auto, with the setpoint at the measurement.
  const integralState = integral ? [{ symbol: integral.term, iec_type: realType(compiled(catalog, integral.term)), unit: mv.unit,
    minimum: mv.minimum - 10 * span, maximum: mv.maximum + 10 * span,
    initial: { source: "pid-integral" as const, index: 0, inputIndex: 0, gainSymbol: draft.proportionalGain.symbol }, postScan: "bounded" as const }] : [];
  // The previous value starts at the error (zero, with the setpoint at the measurement) or the measurement, so D starts at zero.
  const pvSpan = pv.maximum - pv.minimum;
  const derivativeState = derivative ? [{ symbol: derivative.state, iec_type: realType(compiled(catalog, derivative.state)), unit: pv.unit,
    ...(derivative.on === "error" ? { minimum: -pvSpan, maximum: pvSpan, initial: { source: "weighted-error" as const, index: 0, weight: 1 } }
      : { minimum: pv.minimum, maximum: pv.maximum, initial: { source: "model-output" as const, index: 0 } }), postScan: "bounded" as const }] : [];
  return withRevision({
    schemaVersion: 2, id, name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
    topology: "closed-loop",
    controller: {
      instance: draft.instance, type: controller.type, design: "project", revision: "sha256:unverified", action: draft.action,
      taskPeriodMs: period, modeSignal: draft.mode.symbol, modeWhenTrue: draft.mode.whenTrue,
      manualOutputSignal: manual.symbol, manualOutputType: realType(manual), ...(manual.address ? { manualOutputAddress: manual.address } : {}),
      project: {
        family: draft.law, convention: "project-owned-native-controller",
        initialization: { kind: "declared-state-v1", states: [...integralState, ...derivativeState, ...(draft.startupFlag ? [startupState(draft.startupFlag)] : [])] },
        seed: { kind: "parallel-pid", family: draft.law, proportionalSymbol: draft.proportionalGain.symbol,
          ...(integral ? { integralSymbol: integral.gain.symbol } : {}), ...(derivative ? { derivativeSymbol: derivative.gain.symbol } : {}) },
        // One controlled output: its verification campaign names the error requirement without a channel prefix.
        axes: [], objective: { requirementId: "target-error", direction: "minimize" },
        maximumCandidates: 4, maximumCycles: 50_000, wallBudgetMs: 60_000,
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
      parameters: [gain(draft.proportionalGain, `${mv.unit}/${pv.unit}`, "parallel proportional gain"),
        ...(integral ? [gain(integral.gain, `${mv.unit}/(${pv.unit}*s)`, "parallel integral gain")] : []),
        ...(derivative ? [gain(derivative.gain, `${mv.unit}*s/${pv.unit}`, "parallel derivative gain")] : [])],
    },
  } as ControlLoopDefinition);
}
