import { withActuatorFeedback } from "./transferActuatorFeedback";
import type { ControlLoopDefinition, ControlSignalBinding } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import { engineerVerificationPlan, proportionalParameter, type EngineerRequirements } from "./engineerVerificationPlan";

export interface TransferMember { symbol: string; iec_type: "REAL" | "LREAL" | "BOOL"; unit: string; minimum: number; maximum: number }
export interface TransferBinding {
  loop_id: string; task_name: string; mode: { symbol: string; manual_value: boolean };
  /** Further controllers' modes switched with `mode` at the same task boundary (runtime protocol annex, E6). */
  additional_modes?: { symbol: string; manual_value: boolean }[];
  tracking_outputs?: Readonly<Record<string, string>>;
  states: TransferMember[]; guards: TransferMember[]; output: TransferMember; additional_outputs?: TransferMember[];
}
export interface ProportionalTracking {
  kind: "proportional"; reference_symbol: string; manual_command_symbol: string;
  process_variable: string; gain_symbol: string; align_reference: boolean;
}
export function proportionalTracking(loop: ControlLoopDefinition, binding: TransferBinding, alignReference: boolean): ProportionalTracking {
  return { kind: "proportional", reference_symbol: binding.states[0].symbol, manual_command_symbol: binding.states[1].symbol,
    process_variable: loop.signals.processVariable.symbol, gain_symbol: proportionalParameter(loop).symbol, align_reference: alignReference };
}

export function proportionalTransferBinding(loop: ControlLoopDefinition): TransferBinding {
  const parameters = loop.parameterBinding;
  const reference = loop.signals.setpoint;
  const manual = loop.controller.manualOutputSignal;
  const manualType = loop.controller.manualOutputType;
  proportionalParameter(loop);
  if (!parameters || !reference || !manual || !manualType || manualType === "BOOL") {
    throw new Error("Configure the compiled reference, mode and manual-command type before Auto commissioning.");
  }
  const member = (signal: ControlSignalBinding): TransferMember => {
    if (signal.iecType !== "REAL" && signal.iecType !== "LREAL") throw new Error("Auto commissioning requires compiled REAL or LREAL primary signals.");
    return { symbol: signal.symbol, iec_type: signal.iecType, unit: signal.unit,
      minimum: signal.range.minimum, maximum: signal.range.maximum };
  };
  const output = { ...member(loop.signals.manipulatedVariable), minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum };
  return withActuatorFeedback(loop, { loop_id: parameters.loop_id, task_name: parameters.task_name, mode: parameters.mode,
    states: [member(reference), { ...output, symbol: manual, iec_type: manualType }],
    guards: [...parameters.parameters.map(parameter => ({ symbol: parameter.symbol, iec_type: parameter.iec_type,
      unit: parameter.unit, minimum: parameter.minimum, maximum: parameter.maximum })), member(loop.signals.processVariable)], output });
}

export function transferTolerance(loop: ControlLoopDefinition, manual?: number): number {
  const span = loop.limits.outputMaximum - loop.limits.outputMinimum;
  if (manual !== undefined && (!Number.isFinite(manual) || manual < 0 || manual > span)) {
    throw new Error("Auto-entry tolerance must be between zero and the declared output span.");
  }
  return manual ?? 1e-6 * Math.max(1, span);
}

/** An Auto entry's first output also moves by the controller's response in one execution to measurement noise n and to the
 * error e it enters with (Specification 35, A6.4). Without a reviewed tolerance, allow Kp·2n from P on Δy, Ki·Δt·(|e| + n)
 * from I, and Kd·4n/Δt from D on the change of the sampled rate. */
export function noiseAwareTransferTolerance(loop: ControlLoopDefinition, reviewed: number | undefined,
  proportionalGain: number | undefined, processNoise: number | undefined,
  others: { integralGain?: number; derivativeGain?: number; cycleSeconds?: number; entryError?: number } = {}): number {
  if (reviewed !== undefined || !processNoise || proportionalGain === undefined || !Number.isFinite(proportionalGain)) return transferTolerance(loop, reviewed);
  const cycle = others.cycleSeconds !== undefined && others.cycleSeconds > 0 ? others.cycleSeconds : undefined;
  const error = others.entryError !== undefined && Number.isFinite(others.entryError) ? Math.abs(others.entryError) : 0;
  const integral = cycle && Number.isFinite(others.integralGain) ? Math.abs(others.integralGain!) * cycle * (error + processNoise) : 0;
  const derivative = cycle && Number.isFinite(others.derivativeGain) ? 4 * Math.abs(others.derivativeGain!) * processNoise / cycle : 0;
  return Math.max(transferTolerance(loop), 2 * Math.abs(proportionalGain) * processNoise + integral + derivative);
}

export function roundedTransferValue(member: TransferMember, value: number): number {
  if (member.iec_type === "BOOL" && value !== 0 && value !== 1) throw new Error("BOOL transfer state must be 0 or 1.");
  const rounded = member.iec_type === "REAL" ? Math.fround(value) : value;
  if (!Number.isFinite(rounded) || rounded < member.minimum || rounded > member.maximum) {
    throw new Error(`${member.symbol} is outside its declared range after IEC rounding.`);
  }
  return rounded;
}

export function proportionalTrackingReference(binding: TransferBinding, pv: number, mv: number, gain: number): number {
  if (!Number.isFinite(gain) || gain === 0) throw new Error("A zero or invalid P gain cannot align the reference for Auto entry.");
  return roundedTransferValue(binding.states[0], pv + mv / gain);
}

/** Actual ST executes Manual scans before simultaneous reference/mode events. */
export function proportionalInitializationPlan(loop: ControlLoopDefinition, model: GeneralModel,
  task: EngineerRequirements, values: Readonly<Record<string, number>>) {
  const binding = proportionalTransferBinding(loop);
  const parameter = proportionalParameter(loop);
  const gain = values[parameter.symbol];
  const mv = model.operatingPoint?.inputs[0] ?? 0;
  const pv = model.operatingPoint?.outputs[0] ?? 0;
  const reference = proportionalTrackingReference(binding, pv, mv, gain);
  const base = engineerVerificationPlan(loop, model, task, values);
  const step = base.request.step_ms;
  const duration = step * 6;
  const transferAt = step * 3;
  const window = { start: 0, end: duration / 1000 };
  const tolerance = transferTolerance(loop, task.transferTolerance);
  return { reference, tolerance, request: { ...base.request, record_every: undefined, duration_ms: duration, max_total_cycles: 7,
    initial_values: { ...base.request.initial_values, [binding.mode.symbol]: Number(binding.mode.manual_value),
      [binding.states[0].symbol]: pv, [binding.states[1].symbol]: roundedTransferValue(binding.states[1], mv) },
    observations: [...base.request.observations, { symbol: binding.mode.symbol, unit: "1" }],
    events: [{ at_ms: transferAt, symbol: binding.states[0].symbol, value: reference },
      { at_ms: transferAt, symbol: binding.mode.symbol, value: Number(!binding.mode.manual_value) }],
  }, requirements: [
    { id: "transfer-continuity", kind: "bounds" as const, channel: binding.output.symbol, window,
      minimum: mv - tolerance, maximum: mv + tolerance },
    { id: "auto-mode", kind: "bounds" as const, channel: binding.mode.symbol,
      window: { start: transferAt / 1000, end: window.end }, minimum: Number(!binding.mode.manual_value), maximum: Number(!binding.mode.manual_value) },
    ...base.requirements.filter(requirement => requirement.kind === "bounds"),
  ].map(requirement => ({ ...requirement, window: requirement.id === "auto-mode" ? requirement.window : window })) };
}
