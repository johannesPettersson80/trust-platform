import { displayEngineeringUnit, formatEngineeringNumber } from "./chartContract";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { GeneralModel } from "./generalModel";
import { connectedModelChannels } from "./loopChannels";
import type { ControlLoopDefinition } from "./loopDefinition";
import { MainWorkflowError } from "./mainWorkflowFailure";
import { noiseAwareTransferTolerance, transferTolerance, type TransferBinding } from "./proportionalTransfer";
import { decodePidLaw, decodeProportionalLaw } from "./controllerLaw";
import { linearTransferModel } from "./linearTransferModel";
import { RuntimeReadingClock } from "./runtimeDuration";
import type { FoundMode } from "./foundModeReturn";

/**
 * A cascade's inner loop applies while its outer controller is in Manual, holding the inner setpoint (Specification 35 §3).
 * The transfer guards the outer mode, so the runtime also refuses the transfer if it changes before the task boundary.
 */
export function withReferenceHoldGuard(loop: ControlLoopDefinition, binding: TransferBinding): TransferBinding {
  const hold = loop.cascade?.role === "inner" ? loop.cascade.referenceHold : undefined;
  if (!hold || binding.guards.some(guard => guard.symbol.toUpperCase() === hold.modeSymbol.toUpperCase())) return binding;
  return { ...binding, guards: [...binding.guards, { symbol: hold.modeSymbol, iec_type: "BOOL", unit: "1", minimum: 0, maximum: 1 }] };
}

/** Refuses Apply of an inner cascade loop, before any wait or write, while its outer controller is not in Manual. */
export function assertReferenceHeld(loop: ControlLoopDefinition, values: Readonly<Record<string, unknown>>): void {
  const hold = loop.cascade?.role === "inner" ? loop.cascade.referenceHold : undefined;
  if (!hold || Number(values[hold.modeSymbol]) === Number(hold.manualValue)) return;
  throw new ApplyPrecondition(`Put the outer controller in Manual before applying (${hold.modeSymbol}): its output is this loop's setpoint.`);
}

/** Apply did not start because a condition it needs does not hold yet; nothing was written, so the message stands alone. */
export class ApplyPrecondition extends MainWorkflowError {
  constructor(message: string) { super(message, { cause: "settings", nextAction: "reapply" }); }
}

/**
 * The plant inputs Apply compares with the validated operating point (Specification 35, B6.2). A loop found in Manual holds
 * its plant input as a setting, so it must already be the validated one. In Auto the plant input is the running controller's
 * output, which moves with every measurement (A6.4): it is not compared, and the transfer starts from wherever it is.
 */
export function comparedPlantInputs(symbols: readonly string[], foundMode: FoundMode | undefined): readonly string[] {
  return foundMode === "auto" ? [] : symbols;
}

/**
 * Apply starts the validated controller from the selected model's operating point (Specification 35, B6.2). Plant inputs must
 * match within `inputTolerance`, the Auto-entry allowance of that point (A6.4), by default the exact transfer tolerance;
 * measurements count as settled inside the requirement settling band.
 */
export function assertCommissioningStartingPoint(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  inputSymbols: readonly string[], outputSymbols: readonly string[], values: Readonly<Record<string, unknown>>,
  inputTolerance = transferTolerance(loop, task.transferTolerance)): void {
  const unsettled = startingPointRows(loop, model, task, inputSymbols, outputSymbols, values, inputTolerance).find(row => !row.settled);
  if (unsettled) throw unsettledError(loop, unsettled);
}

/** The user ended Apply's wait for a settled process; nothing was written. */
export class StartingPointWaitCancelled extends ApplyPrecondition {
  constructor() { super("Apply cancelled while waiting for the process to settle; nothing was written."); }
}

export interface StartingPointWait {
  progress(text: string): Promise<void> | void;
  cancelled(): boolean;
  /** The process-time bound of the wait (B6.2); the default is the longer of 60 s and three times the test duration. */
  timeoutMs?: number;
  /** The wall-clock pause between two readings. */
  pollMs?: number;
  /** This receiver's wall clock for the connection-loss rule (A3.4); tests inject it. */
  wallClockMs?: () => number;
}

/**
 * Waits until the process rests at the validated operating point (Specification 35, B6.2). A plant input that differs cannot
 * settle by itself and is refused at once. Measured outputs outside their settling bands are waited for: the bound is measured
 * in process time from the runtime's own timestamps, and the wait ends at once when no newer runtime value arrives within the
 * connection-loss timeout. Every output is shown with its validated and its current value. Nothing is written while waiting.
 */
export async function awaitCommissioningStartingPoint<T extends { values: Readonly<Record<string, unknown>>; runtime_time_ns?: number }>(
  loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements, inputSymbols: readonly string[], outputSymbols: readonly string[],
  read: () => Promise<T>, wait: StartingPointWait, inputTolerance = transferTolerance(loop, task.transferTolerance)): Promise<T> {
  const boundMs = wait.timeoutMs ?? Math.max(60_000, 3000 * task.durationSeconds);
  const clock = new RuntimeReadingClock(wait.wallClockMs);
  for (;;) {
    if (wait.cancelled()) throw new StartingPointWaitCancelled();
    const observed = await read();
    const processMs = clock.observe(observed);
    const rows = startingPointRows(loop, model, task, inputSymbols, outputSymbols, observed.values, inputTolerance);
    const refused = rows.find(row => row.input && !row.settled);
    if (refused) throw unsettledError(loop, refused);
    const outputs = rows.filter(row => !row.input);
    const waiting = outputs.find(row => !row.settled);
    if (!waiting) return observed;
    if (processMs > boundMs) throw settleTimeoutError(loop, waiting, boundMs);
    await wait.progress(progressText(loop, outputs));
    clock.ensureFresh();
    await new Promise(resolve => setTimeout(resolve, wait.pollMs ?? 500));
  }
}

/** One plant input or measured output against its validated point. */
interface StartingPointRow {
  symbol: string;
  observed: number;
  expected: number;
  tolerance: number;
  input: boolean;
  settled: boolean;
}

/** Every plant input and every measured output against its validated point, plant inputs first. */
function startingPointRows(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  inputSymbols: readonly string[], outputSymbols: readonly string[], values: Readonly<Record<string, unknown>>,
  inputTolerance: number): StartingPointRow[] {
  const row = (symbol: string, expected: number, tolerance: number, input: boolean, restsWhereItIs: boolean): StartingPointRow => {
    const observed = values[symbol];
    if (typeof observed !== "number" || !Number.isFinite(observed)) throw new Error(`The runtime did not return a finite ${symbol} value.`);
    return { symbol, observed, expected, tolerance, input, settled: restsWhereItIs || Math.abs(observed - expected) <= tolerance };
  };
  return [
    ...inputSymbols.map((symbol, index) => row(symbol, model.operatingPoint?.inputs[index] ?? 0, inputTolerance, true, false)),
    // An integrating output rests wherever its inflow balances: only the plant input decides whether it is at rest.
    ...outputSymbols.map((symbol, index) => row(symbol, model.operatingPoint?.outputs[index] ?? 0, settledBand(model, task, index), false,
      integratingOutput(model, index))),
  ];
}

/** A plant input away from its validated point is refused at once (B6.7: none until it is back); an output is waited for. */
function unsettledError(loop: ControlLoopDefinition, row: StartingPointRow): MainWorkflowError {
  const show = shown(loop, row.symbol);
  return new MainWorkflowError(`${row.symbol} is ${show(row.observed)}, but the settings were validated from ${show(row.expected)} ± ${show(row.tolerance)}. `
    + (row.input ? `Return the manual output to ${show(row.expected)} and let the process settle before applying.` : "Wait for the process to settle there before applying."),
  { cause: "settings", nextAction: row.input ? "none" : "wait" });
}

/** An output that has not settled when the process-time bound ends: nothing was written, and validation starts again from here. */
function settleTimeoutError(loop: ControlLoopDefinition, row: StartingPointRow, boundMs: number): MainWorkflowError {
  const show = shown(loop, row.symbol);
  return new MainWorkflowError(`${row.symbol} did not settle at ${show(row.expected)} ± ${show(row.tolerance)} within `
    + `${formatEngineeringNumber(boundMs / 1000)} s of process time; it is ${show(row.observed)}. `
    + "Apply again once it has settled there, or validate again from its current operating point.", { cause: "settings", nextAction: "validate" });
}

/** Each measured output with its validated value and its current value. */
function progressText(loop: ControlLoopDefinition, outputs: readonly StartingPointRow[]): string {
  const lines = outputs.map(row => {
    const show = shown(loop, row.symbol);
    const target = `${show(row.expected)} ± ${show(row.tolerance)}`;
    return row.settled
      ? `${row.symbol} settled at ${target} (now ${show(row.observed)})`
      : `Waiting for ${row.symbol} to settle at ${target} (now ${show(row.observed)})`;
  });
  return `${lines.join("; ")}…`;
}

/**
 * The first Auto output's allowed change (Specification 35, A6.4 and §12). A PID or P law moves in its first execution by
 * its response to the measured noise `processNoise` and to the error it enters with. A state-feedback law has no retained
 * state that makes entry bumpless; its references follow their measurements, so its first output moves by
 * Σ|Kᵢⱼ|·bⱼ + Σ|Nᵢₖ|·bₖ for the deviations the starting point accepts. A reviewed tolerance replaces both.
 */
export function autoEntryTolerance(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  values: Readonly<Record<string, number>>, processNoise?: number, entryError?: number): number {
  const exact = transferTolerance(loop, task.transferTolerance);
  const law = loop.controller.stateFeedback;
  if (task.transferTolerance !== undefined) return exact;
  if (!law) return lawEntryTolerance(loop, values, processNoise, entryError);
  if (model.kind === "transfer-function") return exact;
  // A state is a measured output when its column of C selects exactly that output; any other state has no accepted deviation.
  const stateBands = model.states.map((_, state) => {
    const selecting = model.c.flatMap((row, output) => row[state] !== 0 ? [output] : []);
    const output = selecting.length === 1 && model.c[selecting[0]][state] === 1
      && model.c[selecting[0]].every((value, column) => column === state || value === 0) ? selecting[0] : undefined;
    return output === undefined ? NaN : settledBand(model, task, output);
  });
  const controlled = connectedModelChannels(loop, model).outputs.flatMap((output, index) => output.reference ? [index] : []);
  const response = (symbols: readonly string[], bands: readonly number[]) =>
    symbols.reduce((sum, symbol, index) => sum + Math.abs(values[symbol]) * bands[index], 0);
  const allowance = Math.max(...law.gainSymbols.map((row, input) => response(row, stateBands)
    + response(law.referenceGainSymbols[input] ?? [], controlled.map(index => settledBand(model, task, index)))));
  return Number.isFinite(allowance) ? Math.max(exact, allowance) : exact;
}

/** The error a controller enters Auto with: its setpoint less its measurement, when the loop has a setpoint (A6.4). */
export function autoEntryError(loop: ControlLoopDefinition, values: Readonly<Record<string, unknown>>): number | undefined {
  const setpoint = loop.signals.setpoint ? values[loop.signals.setpoint.symbol] : undefined;
  const measured = values[loop.signals.processVariable.symbol];
  return typeof setpoint === "number" && typeof measured === "number" ? setpoint - measured : undefined;
}

/** A PID's or P law's A6.4 allowance with the gains `values` holds; other laws keep the exact check. */
function lawEntryTolerance(loop: ControlLoopDefinition, values: Readonly<Record<string, number>>, processNoise?: number,
  entryError?: number): number {
  const cycleSeconds = loop.controller.taskPeriodMs / 1000;
  const pid = decodePidLaw(loop.controller.law, loop.parameterBinding);
  if (pid) {
    return noiseAwareTransferTolerance(loop, undefined, values[pid.proportionalGainSymbol], processNoise,
      { integralGain: values[pid.integralGainSymbol], derivativeGain: values[pid.derivativeGainSymbol], cycleSeconds, entryError });
  }
  const proportional = decodeProportionalLaw(loop.controller.law, loop.parameterBinding);
  return proportional ? noiseAwareTransferTolerance(loop, undefined, values[proportional.gainSymbol], processNoise, { cycleSeconds, entryError })
    : transferTolerance(loop);
}

/** The deviation from the validated operating point that counts as settled for model output `index`. */
function settledBand(model: GeneralModel, task: EngineerRequirements, index: number): number {
  return task.channelRequirements?.[model.outputs[index]?.id]?.settlingBand ?? task.settlingBand;
}

function signalUnit(loop: ControlLoopDefinition, symbol: string): string {
  const signals = [loop.signals.processVariable, loop.signals.manipulatedVariable, loop.signals.setpoint, ...(loop.signals.additional ?? [])];
  const unit = displayEngineeringUnit(signals.find(signal => signal?.symbol === symbol)?.unit ?? "");
  return unit ? ` ${unit}` : "";
}

/** Whether a model output integrates its input: a SISO model with a pole at s = 0. */
function integratingOutput(model: GeneralModel, index: number): boolean {
  if (index !== 0 || model.inputs.length !== 1 || model.outputs.length !== 1) return false;
  try {
    const transfer = linearTransferModel(model);
    return transfer.denominator.length > 1 && transfer.denominator[transfer.denominator.length - 1] === 0;
  } catch { return false; }
}

/**
 * Takes an integrating output's present value as its operating point (Specification 35 §12): its model does not depend on
 * the level, so the validated step and the Auto entry start from wherever the process rests. Other outputs keep theirs.
 */
export function anchorIntegratingOutputs(model: GeneralModel, values: Readonly<Record<string, unknown>>,
  outputSymbols: readonly string[]): GeneralModel {
  if (!model.operatingPoint) return model;
  let changed = false;
  const outputs = model.operatingPoint.outputs.map((value, index) => {
    const present = values[outputSymbols[index] ?? ""];
    if (!integratingOutput(model, index) || typeof present !== "number" || !Number.isFinite(present)) return value;
    changed = true;
    return present;
  });
  return changed ? { ...model, operatingPoint: { ...model.operatingPoint, outputs } } : model;
}

function shown(loop: ControlLoopDefinition, symbol: string): (value: number) => string {
  const unit = signalUnit(loop, symbol);
  return value => `${formatEngineeringNumber(value)}${unit}`;
}
