import type { ParameterBinding } from "./parameterBinding";
import { randomUUID } from "crypto";
import type { LoopMessageScope } from "./messages";
import { isSavedFiniteSearch, searchSummary, type SavedFiniteSearch } from "./finiteSearchPersistence";

export interface SearchConnection { source: "simulation" | "physical"; request: (operation: string, params: Record<string, unknown>) => Promise<unknown> }
type Review = { id: string; connection: SearchConnection; request: Record<string, unknown> };
type ActiveJob = { connection: SearchConnection; id: string; revision: string };

export class FiniteSearchSession {
  private review: Review | undefined;
  private active: ActiveJob | undefined;
  private saved: SavedFiniteSearch | undefined;
  private generation = 0;
  private reviewGeneration = 0;
  private starting = false;
  constructor(private readonly connect: () => Promise<SearchConnection>, private readonly draft?: () => Promise<Record<string, unknown>>) {}

  async handle(message: unknown, scope: LoopMessageScope, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== scope.projectId || message.editorSessionId !== scope.editorSessionId ||
        message.loopId !== scope.loopId || message.expectedLoopRevision !== scope.loopRevision || typeof message.messageId !== "string" ||
        !record(message.payload) || typeof message.kind !== "string" || !message.kind.startsWith("control-studio.search.")) return false;
    const action = message.kind.slice("control-studio.search.".length);
    if (!["inspect", "review", "start", "cancel", "invalidate", "select"].includes(action)) return false;
    const generation = this.generation;
    const reviewAction = ["inspect", "review", "invalidate"].includes(action);
    const reviewGeneration = reviewAction ? ++this.reviewGeneration : this.reviewGeneration;
    const current = () => generation === this.generation && (!reviewAction || reviewGeneration === this.reviewGeneration);
    const respond = async (payload: Record<string, unknown>) => { if (current()) await post(payload); };
    try {
      if (action === "invalidate") { this.review = undefined; this.saved = undefined; await respond({ status: "edited", reason: "Search inputs changed. Review again before starting." }); return true; }
      if (action === "cancel") {
        if (this.active) await this.active.connection.request("control.search.cancel", this.identity(this.active, false));
        await respond({ status: "cancelling", reason: "Cancellation requested; waiting for the worker to stop." }); return true;
      }
      if (action === "select") {
        if (!this.saved || !this.saved.result.ranking.includes(message.payload.index as number)) throw new Error("Select a qualifying completed trial.");
        this.saved.selectedIndex = message.payload.index as number;
        await respond({ status: "complete", result: searchSummary(this.saved) }); return true;
      }
      if (this.active || this.starting) throw new Error("A search is already running. Cancel or wait for it to finish.");
      if (action === "start") {
        const review = this.review; this.review = undefined;
        if (!review || review.id !== message.payload.reviewId) throw new Error("This search review expired. Review the exact inputs again.");
        this.saved = undefined; this.starting = true;
        try {
          const response = await review.connection.request("control.search.start", review.request);
          if (!record(response) || typeof response.job_id !== "string" || response.project_revision !== review.request.expected_project_revision) throw new Error("Runtime returned an invalid search identity.");
          const job: ActiveJob = { connection: review.connection, id: response.job_id, revision: response.project_revision as string };
          this.active = job;
          if (generation !== this.generation) { await job.connection.request("control.search.cancel", this.identity(job, false)); this.active = undefined; return true; }
          await respond({ status: "running", reason: "Executing the reviewed ST trials in isolated simulation." });
          for (let attempt = 0; attempt < 300; attempt++) {
            const progress = await job.connection.request("control.search.status", this.identity(job, false));
            if (generation !== this.generation) return true;
            if (!record(progress) || progress.job_id !== job.id || progress.project_revision !== job.revision) throw new Error("Search identity changed during execution.");
            if (["completed", "cancelled", "failed"].includes(String(progress.status))) {
              const result = await job.connection.request("control.search.status", this.identity(job, true));
              if (generation !== this.generation) return true;
              const saved = { origin: "runtime-isolated-simulation", request: review.request, result };
              if (!isSavedFiniteSearch(saved)) throw new Error("Runtime returned invalid or unmatched search evidence.");
              this.saved = saved; this.active = undefined;
              await respond({ status: "complete", result: searchSummary(saved) }); return true;
            }
            if (generation !== this.generation) return true;
            await respond({ status: "running", result: progress });
            await new Promise(resolve => setTimeout(resolve, 250));
          }
          await job.connection.request("control.search.cancel", this.identity(job, false));
          throw new Error("Search monitoring timed out. Cancellation requested; inspect the runtime before retrying.");
        } finally { this.starting = false; }
      }
      this.review = undefined;
      const connection = await this.connect();
      if (connection.source !== "simulation") throw new Error("Start the local simulator before isolated parameter search.");
      const readiness = await connection.request("control.search.status", {});
      if (!record(readiness) || readiness.ready !== true || typeof readiness.project_revision !== "string") throw new Error(record(readiness) && typeof readiness.reason === "string" ? readiness.reason : "The loaded ST and simulated plant are not ready for search.");
      if (!current()) return true;
      if (action === "inspect") { await respond({ status: "ready", readiness, ...(this.draft ? { draft: await this.draft() } : {}) }); return true; }
      if (typeof message.payload.json !== "string" || message.payload.json.length > 65536) throw new Error("Enter a search request of at most 64 kB.");
      const parsed = JSON.parse(message.payload.json);
      if (!record(parsed) || (parsed.execution !== undefined && parsed.execution !== "isolated-simulation") ||
          (parsed.expected_project_revision !== undefined && parsed.expected_project_revision !== readiness.project_revision)) throw new Error("Search must use the inspected isolated-simulation source revision.");
      const request = freeze({ ...parsed, execution: "isolated-simulation", expected_project_revision: readiness.project_revision });
      const review = { id: randomUUID(), connection, request };
      this.review = review;
      await respond({ status: "reviewed", reviewId: review.id, request });
    } catch (error) { await respond({ status: "failed", reason: error instanceof Error ? error.message : String(error) }); }
    return true;
  }

  async selectedParameterValues(binding: ParameterBinding): Promise<Record<string, number>> {
    const saved = this.saved;
    const selected = saved?.selectedIndex;
    if (!saved || selected === undefined) throw new Error("Select a qualifying search trial first.");
    const trial = saved.result.trials.find(item => item.index === selected);
    if (!trial || trial.status !== "qualified") throw new Error("The selected trial did not qualify.");
    const axes = saved.request.grid;
    const initial = saved.request.initial_values;
    if (!Array.isArray(axes) || !record(initial)) throw new Error("The search lacks complete parameter declarations.");
    const values: Record<string, number> = {};
    for (const parameter of binding.parameters) {
      const axis = axes.find(value => record(value) && value.symbol === parameter.symbol);
      const value = Object.prototype.hasOwnProperty.call(trial.parameters, parameter.symbol) ? trial.parameters[parameter.symbol] : initial[parameter.symbol];
      if (value === undefined) throw new Error(`The search is missing a fixed or tuned value for ${parameter.symbol}; complete the trial settings first.`);
      if ((axis && (!record(axis) || axis.unit !== parameter.unit)) || typeof value !== "number" || !Number.isFinite(value) ||
          value < parameter.minimum || value > parameter.maximum) throw new Error(`The selected ${parameter.symbol} value or unit is incompatible with this binding.`);
      values[parameter.symbol] = value;
    }
    const connection = await this.connect();
    if (connection.source !== "simulation") throw new Error("The simulation source changed; review the result before transferring parameters.");
    const readiness = await connection.request("control.search.status", {});
    if (this.saved !== saved || !record(readiness) || readiness.ready !== true || readiness.project_revision !== saved.result.project_revision) {
      throw new Error("The loaded ST or simulated plant source changed since this search.");
    }
    return values;
  }

  snapshot(): SavedFiniteSearch | undefined { return this.saved ? JSON.parse(JSON.stringify(this.saved)) : undefined; }
  restore(saved: SavedFiniteSearch | undefined): void {
    if (saved && !isSavedFiniteSearch(saved)) throw new Error("Saved search evidence is invalid.");
    const restored = saved ? JSON.parse(JSON.stringify(saved)) : undefined;
    this.dispose();
    this.saved = restored;
  }
  dispose(): void {
    this.generation++; this.reviewGeneration++; this.review = undefined; this.saved = undefined;
    if (this.active) void this.active.connection.request("control.search.cancel", this.identity(this.active, false)).catch(() => undefined);
    this.active = undefined;
  }
  private identity(job: ActiveJob, include_results: boolean): Record<string, unknown> { return { job_id: job.id, expected_project_revision: job.revision, include_results }; }
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function freeze<T>(value: T): T { if (value && typeof value === "object") { Object.values(value).forEach(freeze); Object.freeze(value); } return value; }
