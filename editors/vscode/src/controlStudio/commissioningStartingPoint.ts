import { displayEngineeringUnit, formatEngineeringNumber } from "./chartContract";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { GeneralModel } from "./generalModel";
import { connectedModelChannels } from "./loopChannels";
import type { ControlLoopDefinition } from "./loopDefinition";
import { MainWorkflowError } from "./mainWorkflowFailure";
import { transferTolerance, type TransferBinding } from "./proportionalTransfer";
import { linearTransferModel } from "./linearTransferModel";

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
 * Apply starts the validated controller from the selected model's operating point.
 * Commands must match within the reviewed auto-entry tolerance; measurements count
 * as settled inside the requirement settling band, which already allows for noise.
 */
export function assertCommissioningStartingPoint(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  inputSymbols: readonly string[], outputSymbols: readonly string[], values: Readonly<Record<string, unknown>>): void {
  const deviation = startingPointDeviation(loop, model, task, inputSymbols, outputSymbols, values);
  if (deviation) throw unsettled(loop, deviation);
}

/** The user ended Apply's wait for a settled process; nothing was written. */
export class StartingPointWaitCancelled extends ApplyPrecondition {
  constructor() { super("Apply cancelled while waiting for the process to settle; nothing was written."); }
}

export interface StartingPointWait {
  progress(text: string): Promise<void> | void;
  cancelled(): boolean;
  /** How long to wait in total and between readings; the bound defaults to max(60 s, 3 × the test duration). */
  timeoutMs?: number;
  pollMs?: number;
}

/**
 * Waits until the process rests at the validated operating point (Specification 35 §12). A plant input that differs cannot
 * settle by itself and is refused at once; measured outputs outside their settling bands are waited for, bounded and
 * cancellable, with the output's validated and current value as progress. Nothing is written while waiting.
 */
export async function awaitCommissioningStartingPoint<T extends { values: Readonly<Record<string, unknown>> }>(loop: ControlLoopDefinition,
  model: GeneralModel, task: EngineerRequirements, inputSymbols: readonly string[], outputSymbols: readonly string[],
  read: () => Promise<T>, wait: StartingPointWait): Promise<T> {
  const boundMs = wait.timeoutMs ?? Math.max(60_000, 3000 * task.durationSeconds);
  const until = Date.now() + boundMs;
  for (;;) {
    if (wait.cancelled()) throw new StartingPointWaitCancelled();
    const observed = await read();
    const deviation = startingPointDeviation(loop, model, task, inputSymbols, outputSymbols, observed.values);
    if (!deviation) return observed;
    if (deviation.input) throw unsettled(loop, deviation);
    const show = shown(loop, deviation.symbol);
    const target = `${show(deviation.expected)} ± ${show(deviation.tolerance)}`;
    if (Date.now() > until) {
      throw new MainWorkflowError(`${deviation.symbol} did not settle at ${target} within ${formatEngineeringNumber(boundMs / 1000)} s; it is ${show(deviation.observed)}. `
        + "Apply again once it has settled there, or validate again from its current operating point.", { cause: "settings", nextAction: "reapply" });
    }
    await wait.progress(`Waiting for ${deviation.symbol} to settle at ${target} (now ${show(deviation.observed)})…`);
    await new Promise(resolve => setTimeout(resolve, wait.pollMs ?? 500));
  }
}

interface Deviation { symbol: string; observed: number; expected: number; tolerance: number; input: boolean }

function startingPointDeviation(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  inputSymbols: readonly string[], outputSymbols: readonly string[], values: Readonly<Record<string, unknown>>): Deviation | undefined {
  const inputTolerance = transferTolerance(loop, task.transferTolerance);
  const coordinates = [
    ...inputSymbols.map((symbol, index) => ({ symbol, expected: model.operatingPoint?.inputs[index] ?? 0, tolerance: inputTolerance, input: true })),
    // An integrating output rests wherever its inflow balances: only the plant input decides whether it is at rest.
    ...outputSymbols.flatMap((symbol, index) => integratingOutput(model, index) ? []
      : [{ symbol, expected: model.operatingPoint?.outputs[index] ?? 0, tolerance: settledBand(model, task, index), input: false }]),
  ];
  for (const coordinate of coordinates) {
    const observed = values[coordinate.symbol];
    if (typeof observed !== "number" || !Number.isFinite(observed)) throw new Error(`The runtime did not return a finite ${coordinate.symbol} value.`);
    if (Math.abs(observed - coordinate.expected) > coordinate.tolerance) return { ...coordinate, observed };
  }
  return undefined;
}

function unsettled(loop: ControlLoopDefinition, deviation: Deviation): MainWorkflowError {
  const show = shown(loop, deviation.symbol);
  return new MainWorkflowError(`${deviation.symbol} is ${show(deviation.observed)}, but the settings were validated from ${show(deviation.expected)} ± ${show(deviation.tolerance)}. `
    + (deviation.input ? `Return the manual output to ${show(deviation.expected)} and let the process settle before applying.` : "Wait for the process to settle there before applying."),
  { cause: "settings", nextAction: "reapply" });
}

function shown(loop: ControlLoopDefinition, symbol: string): (value: number) => string {
  const unit = signalUnit(loop, symbol);
  return value => `${formatEngineeringNumber(value)}${unit}`;
}

/**
 * The first Auto output's allowed change (Specification 35 §12). A state-feedback law has no retained state that makes
 * entry bumpless; its references follow their measurements, so its first output moves by Σ|Kᵢⱼ|·bⱼ + Σ|Nᵢₖ|·bₖ for the
 * deviations the starting point accepts. A reviewed tolerance replaces this, and other controllers keep the exact check.
 */
export function autoEntryTolerance(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  values: Readonly<Record<string, number>>): number {
  const exact = transferTolerance(loop, task.transferTolerance);
  const law = loop.controller.stateFeedback;
  if (task.transferTolerance !== undefined || !law || model.kind === "transfer-function") return exact;
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
