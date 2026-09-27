import { evaluateInitializationFormula } from "./controllerInitializationFormulas";
import { transferOutputBasis } from "./transferActuatorFeedback";
import { RuntimeControlError } from "../runtimeControlClient";
import type { DeclaredStateTracking } from "./controllerInitialization";
import { randomUUID } from "crypto";
import { roundedTransferValue, type ProportionalTracking, type TransferBinding } from "./proportionalTransfer";
import type { ParallelPidTracking } from "./pidTransfer";
import type { RuntimeRequest } from "./verificationClient";

export interface TransferOwner { id: string; program: string; binding: string }

export type TransferTracking = ProportionalTracking | ParallelPidTracking | DeclaredStateTracking;

export interface TransferStatus {
  program_revision: string; binding_revision: string;
  values: Record<string, number | boolean>;
  status: "idle" | "pending" | "applied" | "rejected" | "failed" | "cancelled";
  reason: string | null;
  supported_tracking?: string[];
  transaction: null | { id: string; program_revision: string; binding_revision: string;
    status: string; reason: string | null; observed_values: Record<string, number | boolean> | null;
    confirmation: "post-st" | "output-image" | null;
    initial_values?: Record<string, number | boolean> | null; applied_values?: Record<string, number> | null };
}

/** A transport and program epoch stay pinned through acknowledgement and recovery. */
export class RuntimeTransferClient {
  private active: TransferOwner | undefined;
  private retainOwner: ((owner: TransferOwner) => Promise<void>) | undefined;
  setOwnerPersistence(retain: (owner: TransferOwner) => Promise<void>): void { this.retainOwner = retain; }
  restoreOwner(owner: TransferOwner): void {
    if (this.active || owner.program !== this.program) throw new Error("Cannot restore a transfer owned by another program or active transaction.");
    this.active = structuredClone(owner);
  }
  readonly outcomes: TransferStatus[] = [];
  constructor(readonly binding: TransferBinding, private readonly program: string, private readonly request: RuntimeRequest) {}

  async status(): Promise<TransferStatus> {
    const value = await this.request("control.transfer.status", { binding: this.binding });
    if (!record(value) || !text(value.program_revision) || !text(value.binding_revision)
      || !["idle", "pending", "applied", "rejected", "failed", "cancelled"].includes(String(value.status))
      || !this.validValues(value.values) || !(value.reason === null || typeof value.reason === "string")
      || (value.supported_tracking !== undefined && (!Array.isArray(value.supported_tracking)
        || value.supported_tracking.length > 16 || !value.supported_tracking.every(text)))) {
      throw new Error("The runtime returned invalid controller-transfer status.");
    }
    const transaction = value.transaction;
    if (transaction !== null && (!record(transaction) || !text(transaction.id) || !text(transaction.program_revision)
      || !text(transaction.binding_revision) || transaction.status !== value.status
      || !(transaction.reason === null || typeof transaction.reason === "string")
      || !(transaction.observed_values === null || this.validValues(transaction.observed_values))
      || (transaction.initial_values != null && !this.validValues(transaction.initial_values))
      || (transaction.applied_values != null && !this.validAppliedValues(transaction.applied_values))
      || ![null, "post-st", "output-image"].includes(transaction.confirmation))) {
      throw new Error("The runtime returned invalid transfer outcome evidence.");
    }
    return value as unknown as TransferStatus;
  }

  async apply(values: Record<string, number>, manual: boolean, tolerance: number, tracking?: TransferTracking, reviewed?: TransferStatus): Promise<TransferStatus> {
    if (this.active) throw new Error("Resolve the owned transfer before starting another.");
    const status = reviewed ?? await this.status();
    if (status.program_revision !== this.program || status.status === "pending") throw new Error("The program changed or another transfer is pending.");
    const active = { id: randomUUID(), program: this.program, binding: status.binding_revision };
    await this.retainOwner?.(active);
    this.active = active;
    // A lost acknowledgement is resolved using the same identity, never another Apply.
    try {
      await this.request("control.transfer.apply", { transaction_id: active.id, binding: this.binding,
        expected_program_revision: active.program, expected_binding_revision: active.binding,
        expected_values: status.values, values, manual, maximum_output_change: tolerance,
        ...(tracking ? { tracking } : {}),
        source: "simulation", physical_approved: false });
    } catch (error) {
      if (error instanceof RuntimeControlError && error.code === "invalid_transfer_request") {
        this.active = undefined;
        throw error;
      }
      const observed = await this.status().catch(() => undefined);
      // Reconcile the same transaction after a transport error. A missing or
      // different identity is unresolved, never permission to submit again.
      if (!observed || observed.transaction?.id !== active.id
        || observed.transaction.program_revision !== active.program
        || observed.transaction.binding_revision !== active.binding) throw error;
    }
    const outcome = await this.finish(active);
    if (outcome.status !== "applied" || outcome.transaction?.confirmation !== "output-image") {
      throw new Error(outcome.reason ?? `Transfer ${outcome.status}.`);
    }
    const expectedMode = manual ? this.binding.mode.manual_value : !this.binding.mode.manual_value;
    const observed = outcome.transaction.observed_values;
    const expected = { ...values };
    if (tracking?.kind === "proportional") {
      const initial = outcome.transaction.initial_values;
      if (!this.validValues(initial)) throw new Error("Boundary tracking inputs were not retained.");
      const member = (symbol: string) => this.binding.states.find(member => member.symbol === symbol)!;
      const held = Number(initial[transferOutputBasis(this.binding, this.binding.output).symbol]);
      expected[tracking.manual_command_symbol] = roundedTransferValue(member(tracking.manual_command_symbol), held);
      if (tracking.align_reference) expected[tracking.reference_symbol] = roundedTransferValue(member(tracking.reference_symbol),
        Number(initial[tracking.process_variable]) + held / Number(initial[tracking.gain_symbol]));
      const applied = outcome.transaction.applied_values;
      if (!applied || Object.keys(applied).length !== Object.keys(expected).length
        || Object.entries(expected).some(([key, value]) => applied[key] !== value)) throw new Error("Applied tracking values disagree with the retained boundary inputs.");
    }
    if (tracking?.kind === "parallel-pid-v1") {
      const applied = outcome.transaction.applied_values;
      if (!applied || Object.keys(applied).length !== this.binding.states.length
        || this.binding.states.some(member => typeof applied[member.symbol] !== "number" || !Number.isFinite(applied[member.symbol]))) {
        throw new Error("Applied PID initialization states are incomplete or nonfinite.");
      }
      expected[tracking.reference_symbol] = applied[tracking.reference_symbol];
      expected[tracking.manual_command_symbol] = applied[tracking.manual_command_symbol];
    }
    if (tracking?.kind === "declared-state-v1") {
      const applied = outcome.transaction.applied_values;
      if (!applied || !this.validAppliedValues(applied)) throw new Error("Declared initialization states were not retained at the task boundary.");
      const initial = outcome.transaction.initial_values;
      if (!initial) throw new Error("The initialization snapshot is unavailable.");
      for (const [symbol, requested] of Object.entries(tracking.initialization_values)) {
        const formula = tracking.initialization_formulas?.[symbol];
        const member = this.binding.states.find(member => member.symbol === symbol)!;
        const expectedInitial = tracking.align_output ? formula ? roundedTransferValue(member, evaluateInitializationFormula(this.binding, formula, initial)) : requested : Number(initial[symbol]);
        if (applied[symbol] !== expectedInitial) throw new Error(`Initialization state ${symbol} was not adopted coherently.`);
      }
      if (tracking.align_output) {
        const referenceMember = this.binding.states.find(member => member.symbol === tracking.reference_symbol)!;
        if (applied[tracking.reference_symbol] !== roundedTransferValue(referenceMember, Number(initial[tracking.process_variable]))) throw new Error("Auto-entry reference did not match the task-boundary measurement.");
      }
      const manualMember = this.binding.states.find(member => member.symbol === tracking.manual_command_symbol)!;
      if (applied[tracking.manual_command_symbol] !== roundedTransferValue(manualMember, Number(initial[transferOutputBasis(this.binding, this.binding.output).symbol]))) throw new Error("Manual output did not track the committed output.");
      for (const channel of tracking.additional_references ?? []) {
        const member = this.binding.states.find(member => member.symbol === channel.reference_symbol)!;
        if (tracking.align_output && applied[channel.reference_symbol] !== roundedTransferValue(member, Number(initial[channel.process_variable]))) throw new Error("An additional reference did not align with its measured process value.");
      }
      for (const channel of tracking.additional_commands ?? []) {
        const member = this.binding.states.find(member => member.symbol === channel.manual_command_symbol)!;
        if (applied[channel.manual_command_symbol] !== roundedTransferValue(member, Number(initial[this.binding.tracking_outputs?.[channel.output_symbol] ?? channel.output_symbol]))) throw new Error("An additional Manual command did not track its committed output.");
      }
      for (const member of this.binding.states) {
        const actual = observed?.[member.symbol];
        if (typeof actual !== "number" || actual < member.minimum || actual > member.maximum) throw new Error(`Observed initialization state ${member.symbol} is invalid.`);
        expected[member.symbol] = tracking.bounded_state_symbols.includes(member.symbol) ? actual : applied[member.symbol];
      }
    }
    const pidPostScanStatesValid = tracking?.kind === "parallel-pid-v1" && this.binding.states.every(member => {
      const value = observed?.[member.symbol];
      return typeof value === "number" && Number.isFinite(value) && value >= member.minimum && value <= member.maximum;
    });
    const exactExpected = tracking?.kind !== "parallel-pid-v1";
    const expectedEntries = tracking?.kind === "parallel-pid-v1"
      ? Object.entries(expected).filter(([key]) => key === tracking.reference_symbol || key === tracking.manual_command_symbol)
      : Object.entries(expected);
    if (!observed || observed[this.binding.mode.symbol] !== expectedMode || (!exactExpected && !pidPostScanStatesValid)
      || expectedEntries.some(([key, value]) => observed[key] !== value)) {
      throw new Error("The observed transfer does not match the reviewed mode and state.");
    }
    return outcome;
  }

  hasOwnedOperation(): boolean { return this.active !== undefined || this.outcomes.length > 0; }

  async cancel(): Promise<void> {
    const active = this.active;
    if (!active) return;
    try {
      const cancelled = await this.request("control.transfer.cancel", { loop_id: this.binding.loop_id, transaction_id: active.id,
        expected_program_revision: active.program });
      if (record(cancelled) && cancelled.before_dispatch === true && cancelled.confirmed === true && cancelled.status === "cancelled"
        && cancelled.transaction_id === active.id && cancelled.program_revision === active.program && cancelled.binding_revision === active.binding) {
        this.active = undefined;
        return;
      }
    } catch {
      // Cancellation acknowledgement can be lost too; terminal status owns the result.
    }
    await this.finish(active);
  }

  private async finish(active: { id: string; program: string; binding: string }): Promise<TransferStatus> {
    const deadline = Date.now() + 10000;
    while (Date.now() < deadline) {
      const status = await this.status();
      const transaction = status.transaction;
      if (!transaction || transaction.id !== active.id || transaction.program_revision !== active.program || transaction.binding_revision !== active.binding) {
        throw new Error("The owned controller-transfer identity changed; confirmation is unresolved.");
      }
      if (status.status !== "pending") {
        this.active = undefined;
        if (!this.outcomes.some(value => value.transaction?.id === active.id)) this.outcomes.push(status);
        return status;
      }
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    throw new Error("Controller transfer confirmation is unresolved. Cancel retries the same owned transaction.");
  }

  private validAppliedValues(value: unknown): boolean {
    return record(value) && Object.keys(value).length === this.binding.states.length
      && this.binding.states.every(member => typeof value[member.symbol] === "number" && Number.isFinite(value[member.symbol]));
  }

  private validValues(value: unknown): value is Record<string, number | boolean> {
    if (!record(value) || typeof value[this.binding.mode.symbol] !== "boolean") return false;
    const members = [...this.binding.states, ...this.binding.guards, this.binding.output, ...(this.binding.additional_outputs ?? [])];
    return Object.keys(value).length === members.length + 1 && members.every(member =>
      typeof value[member.symbol] === "number" && Number.isFinite(value[member.symbol]));
  }
}
function record(value: unknown): value is Record<string, any> { return !!value && typeof value === "object" && !Array.isArray(value); }
function text(value: unknown): value is string { return typeof value === "string" && value.length > 0; }
