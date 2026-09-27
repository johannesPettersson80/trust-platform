import { createHash, randomUUID } from "crypto";
import { evaluateRecordedRun, type RecordedRun, type RunEvaluation, type RunRequirement } from "./runValidation";

export type RuntimeRequest = (operation: string, params: Record<string, unknown>) => Promise<unknown>;
export interface RuntimeVerificationConnection { source: "simulation"; request: RuntimeRequest }
export interface RuntimeVerificationRequest {
  request_id?: string;
  step_ms: number; duration_ms: number; seed: number; max_total_cycles: number; wall_budget_ms: number;
  initial_values: Readonly<Record<string, number>>;
  events: readonly { at_ms: number; symbol: string; value: number }[];
  observations: readonly { symbol: string; unit: string }[];
  parameter_overrides?: readonly { symbol: string; iec_type: "REAL" | "LREAL" | "BOOL"; unit: string; convention: string; minimum: number; maximum: number; value: number }[];
  feedback_relay?: { process_variable: string; command: string; center: number; hysteresis: number; low: number; high: number; restore: number; process_minimum: number; process_maximum: number; positive_gain: boolean; start_ms: number; restore_ms: number; minimum_high_ms?: number; minimum_low_ms?: number };
  simulation_override?: { model_revision: string; model_provenance: string; simulation_toml: string };
}
export interface RuntimeVerificationResult { projectRevision: string; request: Readonly<Record<string, unknown>>; run: RecordedRun; evaluation: RunEvaluation }

export interface RuntimeVerificationAttempt {
  status: "completed" | "failed" | "cancelled" | "cancellation-unconfirmed";
  jobId: string;
  projectRevision: string;
  request: Readonly<Record<string, unknown>>;
  reason: string;
  run?: RecordedRun;
}

type FrozenVerificationRequest = RuntimeVerificationRequest & { execution: "isolated-simulation"; expected_project_revision: string } & Readonly<Record<string, unknown>>;
type ActiveVerification = {
  id: string;
  revision: string;
  request: FrozenVerificationRequest;
  cancelRequested: boolean;
  terminal: boolean;
  cancelPromise?: Promise<void>;
};

/** A trusted isolated run terminated unsuccessfully; its received evidence is retained. */
export class RuntimeVerificationExecutionError extends Error {
  constructor(reason: string) { super(reason); this.name = "RuntimeVerificationExecutionError"; }
}

export class RuntimeVerificationClient {
  private generation = 0;
  private active: ActiveVerification | undefined;
  private disposed = false;
  private busy = false;
  private retained: RuntimeVerificationAttempt | undefined;
  constructor(private readonly connection: RuntimeVerificationConnection,
    private readonly retainOwner?: (attempt: RuntimeVerificationAttempt) => Promise<void>) {}

  async verify(input: RuntimeVerificationRequest, requirements: readonly RunRequirement[]): Promise<RuntimeVerificationResult> {
    if (this.disposed || this.busy || this.active) throw new Error("Verification client is disposed or already running.");
    if (this.connection.source !== "simulation") throw new Error("Verify requires isolated simulation.");
    if (requirements.length === 0) throw new Error("Declare at least one run requirement before verification.");
    const copiedInput = clone(input); const copiedRequirements = clone(requirements);
    const generation = ++this.generation;
    this.busy = true;
    this.retained = undefined;
    let job: ActiveVerification | undefined;
    let terminal = false;
    try {
      const readiness = record(await this.connection.request("control.simulation.run.status", {}));
      this.current(generation);
      const revision = text(readiness?.project_revision);
      if (!readiness || readiness.ready !== true || revision === undefined || readiness.execution !== "isolated-simulation" || readiness.source_basis !== "loaded-runtime-sources") {
        throw new Error(text(readiness?.reason) ?? "The loaded ST and isolated simulation are not ready for verification.");
      }
      if (readiness.supports_request_id !== true) throw new Error("Reload the runtime with recoverable isolated-operation identity support before verification.");
      const id = randomUUID();
      const request = freeze({ ...copiedInput, request_id: id, execution: "isolated-simulation" as const, expected_project_revision: revision }) as FrozenVerificationRequest;
      // Persist the caller-assigned identity before the first Start write. Reopen
      // reconciles this identity; it never repeats Start to recover an acknowledgement.
      const owner: RuntimeVerificationAttempt = { status: "cancellation-unconfirmed", jobId: id, projectRevision: revision,
        request, reason: "An isolated operation was prepared; reconcile its runtime identity before starting further work." };
      await this.retainOwner?.(owner);
      if (generation !== this.generation) {
        this.retained = { ...owner, status: "cancelled", reason: "Cancelled before the isolated Start request was dispatched." };
        throw new Error(this.retained.reason);
      }
      this.retained = owner;
      job = { id, revision, request, cancelRequested: false, terminal: false }; this.active = job;
      try {
        const started = record(await this.connection.request("control.simulation.run.start", request));
        if (!started || started.job_id !== id || started.project_revision !== revision) throw new Error("Runtime returned an invalid verification identity.");
      } catch (error) {
        // A lost start acknowledgement cannot create a second worker.
        const observed = record(await this.connection.request("control.simulation.run.status", identity(job, false)).catch(() => undefined));
        if (!observed || observed.job_id !== id || observed.project_revision !== revision) throw error;
      }
      this.current(generation);
      const deadline = Date.now() + Math.min(60000, Math.max(1, request.wall_budget_ms)) + 5000;
      while (Date.now() < deadline) {
        this.current(generation);
        const status = record(await this.connection.request("control.simulation.run.status", identity(job, false)));
        this.current(generation);
        validateIdentity(status, job);
        if (!status) throw new Error("Runtime returned an invalid verification status.");
        if (["failed", "cancelled"].includes(String(status.status))) {
          terminal = true;
          job.terminal = true;
          const reason = text(status.reason) ?? `Verification ${status.status}.`;
          const attempt: RuntimeVerificationAttempt = { status: status.status as "failed" | "cancelled", jobId: job.id, projectRevision: revision, request, reason };
          this.retained = attempt;
          const details = record(await this.connection.request("control.simulation.run.status", identity(job, true)));
          validateIdentity(details, job);
          if (!details || details.status !== status.status || details.source_basis !== "loaded-runtime-sources"
              || details.io_provenance !== "isolated-simulation-no-physical-drivers") throw new Error("Runtime returned untrusted failed-run evidence.");
          if (details.run !== undefined) {
            const evaluation = evaluateRecordedRun(scopedRun(details.run, job.id), copiedRequirements);
            validateRun(evaluation.run, request);
            if (evaluation.run.evidence !== "ST-executed") throw new Error("Failed-run evidence must come from actual ST execution.");
            attempt.run = evaluation.run;
          }
          throw status.status === "failed" ? new RuntimeVerificationExecutionError(reason) : new Error(reason);
        }
        if (status.status !== "completed") {
          if (!["queued", "running"].includes(String(status.status))) throw new Error("Unknown runtime verification state.");
          await delay(100); continue;
        }
        terminal = true;
        job.terminal = true;
        const completed = record(await this.connection.request("control.simulation.run.status", identity(job, true)));
        this.current(generation);
        validateIdentity(completed, job);
        if (!completed || completed.status !== "completed" || completed.source_basis !== "loaded-runtime-sources" || completed.io_provenance !== "isolated-simulation-no-physical-drivers") throw new Error("Runtime returned incomplete or untrusted verification evidence.");
        const evaluation = evaluateRecordedRun(scopedRun(completed.run, job.id), copiedRequirements);
        validateRun(evaluation.run, request);
        if (evaluation.run.evidence !== "ST-executed") throw new Error("Verification requires ST-executed runtime evidence.");
        this.retained = { status: "completed", jobId: id, projectRevision: revision, request, run: evaluation.run,
          reason: "The isolated run completed; retained evidence does not authorize application." };
        return { projectRevision: revision, request, run: evaluation.run, evaluation };
      }
      throw new Error("Verification monitoring timed out; cancellation requested.");
    } finally {
      if (job && !terminal) await this.confirmCancellation(job);
      if (job && this.active === job && job.terminal) this.active = undefined;
      this.busy = false;
      if (this.retained) await this.retainOwner?.(this.retained);
    }
  }

  restoreCancellation(attempt: RuntimeVerificationAttempt): void {
    if (this.active || this.busy || attempt.status !== "cancellation-unconfirmed" || attempt.request.execution !== "isolated-simulation"
      || attempt.request.expected_project_revision !== attempt.projectRevision) throw new Error("Only a matching unresolved isolated operation can be restored for cancellation.");
    this.retained = clone(attempt);
    this.active = { id: attempt.jobId, revision: attempt.projectRevision, request: freeze(clone(attempt.request)) as FrozenVerificationRequest,
      cancelRequested: false, terminal: false };
  }

  snapshot(): RuntimeVerificationAttempt | undefined {
    return this.retained ? clone(this.retained) : undefined;
  }
  async cancel(): Promise<void> {
    this.generation++;
    if (this.active) await this.confirmCancellation(this.active);
    if (this.active?.terminal) this.active = undefined;
    if (this.retained) await this.retainOwner?.(this.retained);
  }
  dispose(): void { this.disposed = true; void this.cancel().catch(() => undefined); }

  private async confirmCancellation(job: ActiveVerification): Promise<void> {
    if (job.terminal) return;
    if (job.cancelPromise) return job.cancelPromise;
    const confirmation = this.awaitCancellationTerminal(job).catch(error => {
      this.retained = {
        status: "cancellation-unconfirmed", jobId: job.id, projectRevision: job.revision,
        request: job.request, reason: `Cancellation could not be confirmed: ${error instanceof Error ? error.message : String(error)}`,
      };
    }).finally(() => { if (job.cancelPromise === confirmation) job.cancelPromise = undefined; });
    job.cancelPromise = confirmation;
    return confirmation;
  }

  private async awaitCancellationTerminal(job: ActiveVerification): Promise<void> {
    let status: Record<string, unknown> | undefined;
    if (!job.cancelRequested) {
      job.cancelRequested = true;
      try {
        status = record(await this.connection.request("control.simulation.run.cancel", identity(job, false)));
        validateIdentity(status, job);
      } catch (error) {
        job.cancelRequested = false;
        throw error;
      }
    }
    const deadline = Date.now() + 5_000;
    while (true) {
      if (!status) status = record(await this.connection.request("control.simulation.run.status", identity(job, false)));
      validateIdentity(status, job);
      const terminal = String(status?.status);
      if (["completed", "failed", "cancelled"].includes(terminal)) {
        await this.retainCancellationTerminal(job, terminal as "completed" | "failed" | "cancelled", status!);
        return;
      }
      if (!["queued", "running"].includes(terminal)) throw new Error("Runtime returned an unknown cancellation state.");
      if (Date.now() >= deadline) throw new Error("Timed out waiting for the runtime to confirm cancellation.");
      await delay(100);
      status = undefined;
    }
  }

  private async retainCancellationTerminal(
    job: ActiveVerification,
    status: "completed" | "failed" | "cancelled",
    progress: Record<string, unknown>,
  ): Promise<void> {
    const details = record(await this.connection.request("control.simulation.run.status", identity(job, true)));
    validateIdentity(details, job);
    if (!details || details.status !== status || details.source_basis !== "loaded-runtime-sources"
        || details.io_provenance !== "isolated-simulation-no-physical-drivers") {
      throw new Error("Runtime returned untrusted terminal cancellation evidence.");
    }
    const reason = status === "completed"
      ? "Verification completed after cancellation was requested; it was not approved."
      : text(progress.reason) ?? text(details.reason) ?? `Verification ${status}.`;
    const attempt: RuntimeVerificationAttempt = {
      status, jobId: job.id, projectRevision: job.revision, request: job.request, reason,
    };
    if (details.run !== undefined) {
      const decoded = evaluateRecordedRun(scopedRun(details.run, job.id), []);
      validateRun(decoded.run, job.request);
      if (decoded.run.evidence !== "ST-executed") throw new Error("Terminal cancellation run must come from actual ST execution.");
      attempt.run = decoded.run;
    }
    this.retained = attempt;
    job.terminal = true;
  }
  private current(generation: number): void { if (generation !== this.generation) throw new Error("Verification was cancelled or became stale."); }
}

function validateRun(run: RecordedRun, request: RuntimeVerificationRequest & { execution: "isolated-simulation"; expected_project_revision: string }): void {
  const conditions = run.conditions;
  if (canonical((conditions as typeof conditions & { feedbackRelay?: unknown }).feedbackRelay ?? null) !== canonical(request.feedback_relay ?? null)) {
    throw new Error("Recorded feedback policy differs from the frozen acquisition request.");
  }
  if (request.simulation_override) {
    const selected = request.simulation_override;
    const observed = conditions as RecordedRun["conditions"] & { modelProvenance?: unknown; simulationConfigRevision?: unknown };
    const digest = `sha256:${createHash("sha256").update(selected.simulation_toml, "utf8").digest("hex")}`;
    if (conditions.modelRevision !== selected.model_revision || observed.modelProvenance !== selected.model_provenance
        || observed.simulationConfigRevision !== digest) throw new Error("Recorded model or simulation configuration differs from the selected model.");
  }
  if (!text(conditions.programRevision) || !text(conditions.modelRevision) || !text(conditions.parameterSchemaRevision)
      || conditions.durationSeconds !== request.duration_ms / 1000 || conditions.seed !== request.seed
      || canonical(conditions.initialState) !== canonical(request.initial_values)
      || canonical(conditions.events) !== canonical(request.events.map(event => ({ atSeconds: event.at_ms / 1000, channel: event.symbol, value: event.value })))
      || JSON.stringify(run.channels) !== JSON.stringify((request.observations as readonly { symbol: string; unit: string }[]).map(channel => ({ id: channel.symbol, unit: channel.unit })))
      || canonical(run.parameters) !== canonical(Object.fromEntries((request.parameter_overrides ?? []).map(item => [item.symbol, item.value])))) {
    throw new Error("Recorded run does not match the frozen verification request.");
  }
  const expected = Array.from({ length: request.duration_ms / request.step_ms + 1 }, (_, index) => index * request.step_ms / 1000);
  if (JSON.stringify(conditions.sampleTimesSeconds) !== JSON.stringify(expected)) throw new Error("Recorded run sample times do not match the frozen verification request.");
}
function validateIdentity(value: Record<string, unknown> | undefined, job: { id: string; revision: string }): void {
  if (!value || value.job_id !== job.id || value.project_revision !== job.revision) throw new Error("Verification identity or revision changed during execution.");
}
function identity(job: { id: string; revision: string }, include_results: boolean): Record<string, unknown> { return { job_id: job.id, expected_project_revision: job.revision, include_results }; }
function record(value: unknown): Record<string, unknown> | undefined { return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined; }
function text(value: unknown): string | undefined { return typeof value === "string" && value.length > 0 ? value : undefined; }
function freeze<T>(value: T): T { if (value && typeof value === "object") { Object.values(value).forEach(freeze); Object.freeze(value); } return value; }
function clone<T>(value: T): T { return JSON.parse(JSON.stringify(value)) as T; }
function canonical(value: unknown): string { return JSON.stringify(value && typeof value === "object" && !Array.isArray(value) ? Object.fromEntries(Object.keys(value).sort().map(key => [key, JSON.parse(canonical((value as Record<string, unknown>)[key]))])) : Array.isArray(value) ? value.map(item => JSON.parse(canonical(item))) : value); }
function delay(milliseconds: number): Promise<void> { return new Promise(resolve => setTimeout(resolve, milliseconds)); }

/** Worker-local run numbers are namespaced by the confirmed runtime job identity. */
function scopedRun(value: unknown, jobId: string): unknown {
  const run = record(value);
  if (!run || !text(run.id)) throw new Error("Runtime recording has no run identity.");
  return { ...run, id: `${jobId}:${run.id}` };
}
