import { BoundedAnalysisJob } from "./boundedAnalysisJob";
import * as path from "path";
import type { LoopMessageScope } from "./messages";
import { evaluateRecordedRun, type RecordedRun } from "./runValidation";
import { isSavedRunValidation, type SavedRunValidation } from "./runValidationPersistence";
export class RunValidationSession {
  private current: RecordedRun | undefined;
  private candidate: RecordedRun | undefined;
  private saved: SavedRunValidation | undefined;
  private generation = 0;
  private readonly job = new BoundedAnalysisJob();
  async handle(message: unknown, scope: LoopMessageScope, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== scope.projectId || message.editorSessionId !== scope.editorSessionId
      || message.loopId !== scope.loopId || message.expectedLoopRevision !== scope.loopRevision || typeof message.messageId !== "string"
      || !record(message.payload) || typeof message.kind !== "string" || !message.kind.startsWith("control-studio.run-validation.")) return false;
    const action = message.kind.slice("control-studio.run-validation.".length);
    if (!["import-current", "import-candidate", "evaluate", "cancel", "invalidate"].includes(action)) return false;
    const payload = message.payload;
    try {
      if (JSON.stringify(message).length > 2200000) throw new Error("Run import exceeds the 2 MiB limit.");
      this.cancel(); this.saved = undefined;
      if (action === "invalidate") { await post({ status: "edited", reason: "Requirements changed. Evaluate again before saving." }); return true; }
      if (action === "cancel") { await post({ status: "cancelled", reason: "Validation cancelled." }); return true; }
      if (typeof payload.json !== "string" || Buffer.byteLength(payload.json, "utf8") > 2 * 1024 * 1024) throw new Error("Choose a JSON file of at most 2 MiB.");
      const decoded = JSON.parse(payload.json);
      if (action.startsWith("import-")) {
        const run = evaluateRecordedRun(decoded, []).run;
        if (action === "import-current") this.current = run; else this.candidate = run;
        await post({ status: "imported", role: action.slice(7), id: run.id, evidence: run.evidence }); return true;
      }
      if (!this.current) throw new Error("Import a complete current-run contract before evaluation.");
      if (!Array.isArray(decoded) || decoded.length === 0 || decoded.length > 64) throw new Error("Declare a JSON array of 1–64 requirements.");
      if (payload.objective !== undefined && (typeof payload.objective !== "string" || payload.objective.length > 256)) throw new Error("Objective must name one requirement ID.");
      const generation = this.generation;
      await post({ status: "running", reason: "Evaluating imported records in a bounded worker…" });
      if (generation !== this.generation) return true;
      const saved = await this.job.run<unknown>(path.join(__dirname, "runValidationWorker.js"), {
        current: this.current, candidate: this.candidate, requirements: decoded, objective: payload.objective
      });
      if (generation !== this.generation || saved === undefined) return true;
      if (!isSavedRunValidation(saved)) throw new Error("Worker returned invalid validation evidence.");
      this.saved = saved; await post({ status: "complete", saved });
    } catch (error) {
      this.saved = undefined;
      await post({ status: "failed", reason: error instanceof Error ? error.message : String(error) });
    }
    return true;
  }
  snapshot(): SavedRunValidation | undefined { return this.saved ? JSON.parse(JSON.stringify(this.saved)) : undefined; }
  restore(saved: SavedRunValidation | undefined): void {
    if (saved && !isSavedRunValidation(saved)) throw new Error("Saved run validation is invalid.");
    const restored = saved ? JSON.parse(JSON.stringify(saved)) : undefined;
    this.cancel();
    this.saved = restored; this.current = this.saved?.current; this.candidate = this.saved?.candidate;
  }
  dispose(): void { this.cancel(); this.current = undefined; this.candidate = undefined; this.saved = undefined; }
  private cancel(): void { this.generation++; this.job.cancel(); }
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
