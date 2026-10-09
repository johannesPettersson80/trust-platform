import type { ControlLoopDefinition } from "../loopDefinition";
import type { CompiledCatalog } from "./compiledCatalog";
import { binding, measurementBinding, compiled, explicitExperiment, loopId, realType, withRevision, type MeasuredSignal } from "./loopBindings";
import { startupState } from "./pidBindings";
import type { ModeChoice, SignalChoice } from "./pidLoopGenerator";

/** An existing switching controller in the program (Specification 35 §17.1). */
export interface OnOffLoopDraft {
  projectId: string; runtimeId: string; name: string; action: "direct" | "reverse";
  /** The compiled program or function block instance that runs the switching logic. */
  instance: string;
  processVariable: MeasuredSignal; setpoint: SignalChoice;
  /** The numeric output; it switches between its minimum and maximum. */
  manipulatedVariable: MeasuredSignal;
  mode: ModeChoice; manualOutput: string;
  hysteresis: string; bandConvention: "half-band" | "full-band"; minimumOn: string; minimumOff: string;
  /** The BOOL that holds whether the output is on. */
  latch: string;
  /** Seconds spent in the current state, when the program counts them. */
  dwellTimer?: string;
  /** The switched equipment's own minimum ON and OFF times, for example a compressor's restart protection. */
  equipmentMinimumOnSeconds?: number;
  equipmentMinimumOffSeconds?: number;
  experimentStep?: number; experimentDurationSeconds?: number;
  startupFlag?: string;
}

/** A switching loop whose band and minimum ON/OFF times Tune designs and Validate checks on the actual ST. */
export function onOffLoopDefinition(catalog: CompiledCatalog, draft: OnOffLoopDraft): ControlLoopDefinition {
  const controller = catalog.controllers.find(entry => entry.instance === draft.instance);
  if (!controller) throw new Error(`${draft.instance} is not a compiled program or function block instance.`);
  if (controller.tasks.length !== 1 || !(controller.tasks[0].intervalMs > 0)) throw new Error(`${draft.instance} must run in exactly one cyclic task.`);
  const task = controller.tasks[0], period = task.intervalMs, id = loopId(draft.name);
  const pv = draft.processVariable, mv = draft.manipulatedVariable;
  const setpoint: MeasuredSignal = { symbol: draft.setpoint.symbol, unit: draft.setpoint.unit ?? pv.unit,
    minimum: draft.setpoint.minimum ?? pv.minimum, maximum: draft.setpoint.maximum ?? pv.maximum };
  const manual = compiled(catalog, draft.manualOutput);
  const latch = compiled(catalog, draft.latch);
  if (latch.type !== "BOOL") throw new Error(`${draft.latch} must be a BOOL: it holds whether the output is on.`);
  const dwell = draft.dwellTimer ? compiled(catalog, draft.dwellTimer) : undefined;
  const setting = (symbol: string, unit: string, convention: string, maximum: number) =>
    ({ symbol, iec_type: realType(compiled(catalog, symbol)), unit, convention, minimum: 0, maximum });
  return withRevision({
    schemaVersion: 2, id, name: draft.name, projectId: draft.projectId, runtimeId: draft.runtimeId, revision: "sha256:pending",
    topology: "closed-loop",
    controller: {
      instance: draft.instance, type: controller.type, design: "project", revision: "sha256:unverified", action: draft.action,
      taskPeriodMs: period, modeSignal: draft.mode.symbol, modeWhenTrue: draft.mode.whenTrue,
      manualOutputSignal: manual.symbol, manualOutputType: realType(manual), ...(manual.address ? { manualOutputAddress: manual.address } : {}),
      onOff: {
        hysteresisSymbol: draft.hysteresis, minimumOnTimeSymbol: draft.minimumOn, minimumOffTimeSymbol: draft.minimumOff,
        bandConvention: draft.bandConvention, lowOutput: mv.minimum, highOutput: mv.maximum,
        initialization: { kind: "declared-state-v1", states: [
          { symbol: latch.symbol, iec_type: "BOOL", unit: "1", minimum: 0, maximum: 1,
            initial: { source: "model-input-is", index: 0, value: mv.maximum }, postScan: "bounded" },
          ...(dwell ? [{ symbol: dwell.symbol, iec_type: realType(dwell), unit: "s", minimum: 0, maximum: 1e6, initial: 0, postScan: "bounded" }] : []),
          ...(draft.startupFlag ? [startupState(draft.startupFlag)] : []),
        ] },
      },
    },
    signals: {
      setpoint: binding(catalog, setpoint, "read"),
      processVariable: measurementBinding(catalog, pv),
      manipulatedVariable: binding(catalog, mv, "write"),
    },
    // The equipment's dwell limits belong to the output channel; Tune never designs shorter minimum times.
    channels: {
      inputs: [{ modelChannelId: mv.symbol, signalId: "manipulatedVariable", actuator: { kind: "on-off", minimum: mv.minimum, maximum: mv.maximum,
        ...(draft.equipmentMinimumOnSeconds !== undefined ? { minimumOnSeconds: draft.equipmentMinimumOnSeconds } : {}),
        ...(draft.equipmentMinimumOffSeconds !== undefined ? { minimumOffSeconds: draft.equipmentMinimumOffSeconds } : {}) } }],
      outputs: [{ modelChannelId: pv.symbol, signalId: "processVariable", setpointSignalId: "setpoint" }],
    },
    limits: { outputMinimum: mv.minimum, outputMaximum: mv.maximum },
    observedSampleTiming: { nominalPeriodMs: period },
    // Identification switches between the two levels, as the actuator does in operation.
    ...explicitExperiment(draft.name, { execution: "control-studio", kind: "mv_step" }, draft.experimentStep, draft.experimentDurationSeconds),
    parameterBinding: {
      loop_id: id, task_name: task.name, mode: { symbol: draft.mode.symbol, manual_value: draft.mode.whenTrue === "Manual" },
      parameters: [
        setting(draft.hysteresis, pv.unit, "ON/OFF hysteresis", pv.maximum - pv.minimum),
        setting(draft.minimumOn, "s", "ON/OFF minimum ON time", 3600),
        setting(draft.minimumOff, "s", "ON/OFF minimum OFF time", 3600),
      ],
    },
  } as ControlLoopDefinition);
}
