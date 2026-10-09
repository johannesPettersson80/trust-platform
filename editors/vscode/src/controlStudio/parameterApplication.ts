import { randomUUID } from "crypto";
import { decodeParameterBinding, type ParameterBinding } from "./parameterBinding";
import { PhysicalApprovalRequired } from "./runtimeTarget";
import { confirmationTiming } from "./confirmationWindow";

export interface ParameterOwner { id: string; program: string; binding: string }

export interface ParameterStatus {
  supports_request_id?: boolean;
  transaction_id?: string | null;
  program_revision: string;
  binding_revision: string;
  active_revision: string;
  values: Record<string, number>;
  status: "idle" | "pending" | "applied" | "rejected";
  previous_revision: string | null;
  previous_values: Record<string, number> | null;
  reason: string | null;
}
export interface ParameterReview { transaction_id?: string; observed: ParameterStatus; values: Readonly<Record<string, number>> }
type Request = (operation: string, params: Record<string, unknown>) => Promise<unknown>;

/** One instance owns one immutable runtime transport and one compiled binding. */
export class ParameterApplicationClient {
  private readonly binding: ParameterBinding;
  private readonly reviews = new WeakSet<ParameterReview>();
  private readonly rollbacks = new WeakSet<ParameterReview>();
  constructor(binding: ParameterBinding, private readonly source: "simulation" | "physical", private readonly request: Request) {
    if (!decodeParameterBinding(binding, binding.loop_id)) throw new Error("Invalid parameter binding.");
    this.binding = JSON.parse(JSON.stringify(binding));
  }

  async status(): Promise<ParameterStatus> {
    const value = await this.request("control.parameters.status", { binding: this.binding });
    if (!record(value) || !["program_revision", "binding_revision", "active_revision"].every(key => typeof value[key] === "string" && value[key]) ||
        (value.supports_request_id !== undefined && typeof value.supports_request_id !== "boolean") ||
        (value.transaction_id !== undefined && value.transaction_id !== null && !(typeof value.transaction_id === "string" && value.transaction_id.length > 0)) ||
        !["idle", "pending", "applied", "rejected"].includes(String(value.status)) ||
        !this.validValues(value.values) || !(value.previous_revision === null || typeof value.previous_revision === "string") ||
        !(value.previous_values === null || this.validValues(value.previous_values)) ||
        !(value.reason === null || typeof value.reason === "string")) throw new Error("Runtime returned invalid parameter status.");
    return value as unknown as ParameterStatus;
  }

  async review(input: Record<string, number>): Promise<ParameterReview> {
    if (!this.validValues(input)) throw new Error("Provide exactly the declared finite parameter values.");
    const values: Record<string, number> = {};
    for (const parameter of this.binding.parameters) {
      const value = parameter.iec_type === "REAL" ? Math.fround(input[parameter.symbol]) : input[parameter.symbol];
      if (!Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum) throw new Error(`${parameter.symbol} is outside its declared bounds after IEC rounding.`);
      values[parameter.symbol] = value;
    }
    const observed = await this.status();
    if (observed.status === "pending") throw new Error("Wait for the pending parameter transaction before review.");
    Object.freeze(observed.values);
    Object.freeze(observed);
    const review = Object.freeze({ transaction_id: randomUUID(), observed, values: Object.freeze(values) });
    this.reviews.add(review);
    return review;
  }

  async reviewRollback(): Promise<ParameterReview> {
    const observed = await this.status();
    if (observed.status === "pending" || !observed.previous_revision || !observed.previous_values) throw new Error("No confirmed previous parameter set is available for rollback.");
    Object.freeze(observed.values);
    Object.freeze(observed.previous_values);
    Object.freeze(observed);
    const review = Object.freeze({ transaction_id: randomUUID(), observed, values: observed.previous_values });
    this.rollbacks.add(review);
    return review;
  }

  async rollback(review: ParameterReview, physicalApproved: boolean): Promise<ParameterStatus> {
    if (!this.rollbacks.has(review)) throw new Error("Review the retained rollback values on this runtime first.");
    if (this.source === "physical" && !physicalApproved) throw new PhysicalApprovalRequired("parameter rollback");
    if (!review.transaction_id || review.observed.supports_request_id !== true) throw new Error("Reload the runtime with parameter-operation identity support before rollback.");
    this.rollbacks.delete(review);
    const observed = review.observed;
    try { await this.request("control.parameters.rollback", { transaction_id: review.transaction_id, binding: this.binding,
      expected_program_revision: observed.program_revision, expected_binding_revision: observed.binding_revision,
      expected_active_revision: observed.active_revision, expected_values: observed.values,
      previous_revision: observed.previous_revision, source: this.source, physical_approved: physicalApproved }); }
    catch { return this.confirm(review, true); }
    return this.confirm(review, true);
  }

  async apply(review: ParameterReview, physicalApproved: boolean): Promise<ParameterStatus> {
    if (!this.reviews.has(review)) throw new Error("Review this parameter set on the bound runtime first.");
    if (this.source === "physical" && !physicalApproved) throw new PhysicalApprovalRequired("parameter application");
    if (!review.transaction_id || review.observed.supports_request_id !== true) throw new Error("Reload the runtime with parameter-operation identity support before application.");
    this.reviews.delete(review);
    const observed = review.observed;
    try { await this.request("control.parameters.apply", { transaction_id: review.transaction_id, binding: this.binding,
      expected_program_revision: observed.program_revision, expected_binding_revision: observed.binding_revision,
      expected_active_revision: observed.active_revision, expected_values: observed.values,
      values: review.values, source: this.source, physical_approved: physicalApproved }); }
    catch { return this.confirm(review, false); }
    return this.confirm(review, false);
  }

  async cancelOwner(owner: ParameterOwner): Promise<"before-dispatch" | "terminal"> {
    try {
      const result = await this.request("control.parameters.cancel", { loop_id: this.binding.loop_id, transaction_id: owner.id,
        expected_program_revision: owner.program, expected_binding_revision: owner.binding });
      if (record(result) && result.confirmed === true && result.transaction_id === owner.id
        && result.program_revision === owner.program && result.binding_revision === owner.binding) return result.before_dispatch === true ? "before-dispatch" : "terminal";
    } catch { /* A lost cancellation acknowledgement is resolved from the original identity. */ }
    const timing = await confirmationTiming(this.request, this.binding.task_name);
    const deadline = Date.now() + timing.windowMs;
    while (Date.now() < deadline) {
      const current = await this.status();
      if (current.program_revision !== owner.program || current.binding_revision !== owner.binding || current.transaction_id !== owner.id) throw new Error("The parameter-operation identity remains unresolved.");
      if (current.status !== "pending") return "terminal";
      await new Promise(resolve => setTimeout(resolve, timing.pollMs));
    }
    throw new Error("Parameter cancellation remains unconfirmed; retry the retained operation after reconnecting.");
  }

  private async confirm(review: ParameterReview, rollback: boolean): Promise<ParameterStatus> {
    const observed = review.observed;
    const timing = await confirmationTiming(this.request, this.binding.task_name);
    const deadline = Date.now() + timing.windowMs;
    while (Date.now() < deadline) {
      const status = await this.status();
      if (status.program_revision !== observed.program_revision || status.binding_revision !== observed.binding_revision) throw new Error("Runtime program or parameter binding changed during application.");
      if (status.status === "applied" && status.transaction_id === review.transaction_id && sameParameterValues(status.values, review.values) && (rollback ? status.active_revision === observed.previous_revision : status.previous_revision === observed.active_revision)) return status;
      if (status.status !== "pending") throw new Error(status.reason ?? "The reviewed values were not confirmed after controller execution.");
      await new Promise(resolve => setTimeout(resolve, timing.pollMs));
    }
    throw new Error("Timed out waiting for parameter confirmation; inspect runtime status before retrying.");
  }

  private validValues(value: unknown): value is Record<string, number> {
    return record(value) && Object.keys(value).length === this.binding.parameters.length && this.binding.parameters.every(parameter =>
      Object.prototype.hasOwnProperty.call(value, parameter.symbol) && typeof value[parameter.symbol] === "number" && Number.isFinite(value[parameter.symbol]));
  }
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
export function sameParameterValues(a: Record<string, number>, b: Readonly<Record<string, number>>): boolean { return Object.keys(a).length === Object.keys(b).length && Object.keys(a).every(key => a[key] === b[key]); }

/** Historical results only select a recovery route when native value history still matches. */
export function sameParameterHistory(current: ParameterStatus, saved: ParameterStatus | undefined): boolean {
  return Boolean(saved && current.program_revision === saved.program_revision
    && current.binding_revision === saved.binding_revision && current.active_revision === saved.active_revision
    && current.previous_revision === saved.previous_revision && record(saved.values) && sameParameterValues(current.values, saved.values)
    && (current.previous_values === null && saved.previous_values === null
      || current.previous_values && record(saved.previous_values) && sameParameterValues(current.previous_values, saved.previous_values)));
}
