import type { ControlLoopDefinition } from "../loopDefinition";
import { PID_TYPE, type CompiledCatalog } from "./compiledCatalog";
import { binding, measurementBinding, compiled, identificationDuration, loopId, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { pidParameters, pidStates, startupState } from "./pidBindings";
import type { ModeChoice, SignalChoice } from "./pidLoopGenerator";

export interface PidGroupMemberDraft {
  instance: string; parameterRecord: string; processVariable: MeasuredSignal; manipulatedVariable: MeasuredSignal;
  setpoint: SignalChoice; manualOutput: string;
  /** This member's identification step, in its output's unit (Specification 35 §17.9). */
  experimentStep?: number;
}
/** Several library PIDs tuned, validated and applied together (Specification 35 §17.9); they share one Auto/Manual signal. */
export interface PidGroupDraft {
  projectId: string; runtimeId: string; name: string; action: "direct" | "reverse"; mode: ModeChoice;
  /** The step for members without their own, in the outputs' unit. */
  experimentStep?: number;
  experimentDurationSeconds?: number;
  startupFlag?: string;
  members: PidGroupMemberDraft[];
  /** The program's static decoupler, u = D·v in Auto: variable D[i][j] for plant input i and member j (Specification 35 §17.15). */
  decoupler?: string[][];
}

/** One group loop: each member drives one model input and controls one model output, designed on its own path. Model
 * channels are named after their signals, as Identify names the channels of the model it fits. */
export function pidGroupLoopDefinition(catalog: CompiledCatalog, draft: PidGroupDraft): ControlLoopDefinition {
  if (draft.members.length < 2) throw new Error("A PID group needs at least two controllers.");
  if (new Set(draft.members.map(member => member.instance)).size !== draft.members.length) throw new Error("Choose each controller only once.");
  const tasks = draft.members.map(member => {
    const controller = catalog.controllers.find(entry => entry.instance === member.instance && entry.type.toUpperCase() === PID_TYPE);
    if (!controller) throw new Error(`${member.instance} is not a compiled Control.PID instance.`);
    if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${member.instance} must run in exactly one cyclic task.`);
    return controller.tasks[0];
  });
  if (tasks.some(task => task.name !== tasks[0].name)) throw new Error("All members of a PID group must run in the same task.");
  const task = tasks[0], period = task.intervalMs, id = loopId(draft.name), [first] = draft.members;
  const setpoint = (member: PidGroupMemberDraft): MeasuredSignal => ({ symbol: member.setpoint.symbol, unit: member.setpoint.unit ?? member.processVariable.unit,
    minimum: member.setpoint.minimum ?? member.processVariable.minimum, maximum: member.setpoint.maximum ?? member.processVariable.maximum });
  const manual = draft.members.map(member => compiled(catalog, member.manualOutput));
  const gains = (member: PidGroupMemberDraft) => pidParameters(member.parameterRecord, member.processVariable, member.manipulatedVariable,
    ["ProportionalGain", "IntegralGain", "DerivativeGain"]);
  const decoupler = draft.decoupler;
  if (decoupler && (decoupler.length !== draft.members.length || decoupler.some(row => row.length !== draft.members.length)
    || new Set(decoupler.flat().map(symbol => symbol.toUpperCase())).size !== draft.members.length ** 2)) {
    throw new Error("A decoupler needs one variable per plant input and member, each used once.");
  }
  // Behind a decoupler each PID starts at its share of the present plant inputs, v = D⁻¹·u (§17.15).
  const states = draft.members.map((member, index) => pidStates(member.instance, member.processVariable, member.manipulatedVariable, index)
    .map(state => decoupler && /\.StateValue\.(NominalOutput|SaturatedOutput)$/.test(state.symbol)
      ? { ...state, initial: { source: "decoupled-model-input" as const, index, matrixSymbols: decoupler } } : state));
  const unitRatio = (numerator: string, denominator: string) => numerator === denominator ? "1" : `${numerator}/${denominator}`;
  const decouplerParameters = (decoupler ?? []).flatMap((row, i) => row.map((symbol, j) => ({ symbol, iec_type: realType(compiled(catalog, symbol)),
    unit: unitRatio(draft.members[i].manipulatedVariable.unit, draft.members[j].manipulatedVariable.unit),
    convention: `Decoupler entry for ${draft.members[i].manipulatedVariable.symbol} from ${draft.members[j].instance}`, minimum: -1e3, maximum: 1e3 })));
  const step = (member: PidGroupMemberDraft) => member.experimentStep ?? draft.experimentStep
    ?? (member.manipulatedVariable.maximum - member.manipulatedVariable.minimum) / 10;
  return withRevision({
    schemaVersion: 2, id, name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
    topology: "closed-loop",
    controller: {
      instance: first.instance, type: "Control.PID", design: "project", revision: "sha256:unverified", action: draft.action,
      taskPeriodMs: period, modeSignal: draft.mode.symbol, modeWhenTrue: draft.mode.whenTrue,
      manualOutputSignal: manual[0].symbol, manualOutputType: realType(manual[0]), ...(manual[0].address ? { manualOutputAddress: manual[0].address } : {}),
      project: {
        family: decoupler ? "decoupled-PI" : "paired-PI", convention: "project-owned-native-controller",
        initialization: { kind: "declared-state-v1", states: [...states.flat(), ...(draft.startupFlag ? [startupState(draft.startupFlag)] : [])] },
        seed: decoupler ? { kind: "decoupled-pid", family: "PI", matrixSymbols: decoupler } : { kind: "decentralized-pid", family: "PI" }, axes: [],
        objective: { requirementId: `${first.processVariable.symbol}:target-error`, direction: "minimize" },
        maximumCandidates: 1, maximumCycles: 50_000, wallBudgetMs: 60_000,
      },
      group: { members: draft.members.map((member, index) => ({
        instance: member.instance, type: "Control.PID", inputChannelId: member.manipulatedVariable.symbol, outputChannelId: member.processVariable.symbol,
        modeSymbol: draft.mode.symbol,
        parameterSymbols: gains(member).map(parameter => parameter.symbol),
        applicationStateSymbols: [...states[index].map(state => state.symbol), ...(index === 0 && draft.startupFlag ? [draft.startupFlag] : [])],
      })) },
    },
    signals: {
      setpoint: binding(catalog, setpoint(first), "read"),
      processVariable: measurementBinding(catalog, first.processVariable),
      manipulatedVariable: binding(catalog, first.manipulatedVariable, "write"),
      additional: draft.members.slice(1).flatMap((member, offset) => {
        const channel = `member-${offset + 2}`;
        return [
          { id: `mv-${offset + 2}`, channel, role: "MV" as const, ...binding(catalog, member.manipulatedVariable, "write") },
          { id: `pv-${offset + 2}`, channel, role: "PV" as const, ...measurementBinding(catalog, member.processVariable) },
          { id: `sp-${offset + 2}`, channel, role: "SP" as const, ...binding(catalog, setpoint(member), "read") },
        ];
      }),
    },
    channels: {
      inputs: draft.members.map((member, index) => ({ modelChannelId: member.manipulatedVariable.symbol, signalId: index ? `mv-${index + 1}` : "manipulatedVariable",
        ...(index ? { manualOutputSignal: manual[index].symbol, manualOutputType: realType(manual[index]) } : {}),
        actuator: { kind: "continuous" as const, minimum: member.manipulatedVariable.minimum, maximum: member.manipulatedVariable.maximum },
        experimentStep: step(member) })),
      outputs: draft.members.map((member, index) => ({ modelChannelId: member.processVariable.symbol, signalId: index ? `pv-${index + 1}` : "processVariable",
        setpointSignalId: index ? `sp-${index + 1}` : "setpoint" })),
    },
    limits: { outputMinimum: first.manipulatedVariable.minimum, outputMaximum: first.manipulatedVariable.maximum },
    observedSampleTiming: { nominalPeriodMs: period },
    experiment: { execution: "control-studio", kind: "mv_step", amplitude: step(first),
      durationSeconds: identificationDuration(draft.name, draft.experimentDurationSeconds), seed: 1 },
    parameterBinding: {
      loop_id: id, task_name: task.name, mode: { symbol: draft.mode.symbol, manual_value: draft.mode.whenTrue === "Manual" },
      parameters: [...draft.members.flatMap(gains), ...decouplerParameters],
    },
  } as ControlLoopDefinition);
}
