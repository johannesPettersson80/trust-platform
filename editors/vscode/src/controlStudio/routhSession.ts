import type { DeclaredControlAnalysis } from "./declaredAnalysis";
import type { LoopMessageScope } from "./messages";
import { calculateDeclaredRouth } from "./routhHurwitz";
import { BoundedAnalysisJob } from "./boundedAnalysisJob";
import * as path from "path";
import { isSavedRouthAnalysis, type SavedRouthAnalysis } from "./routhPersistence";
export { isSavedRouthAnalysis, type SavedRouthAnalysis } from "./routhPersistence";

/** Read-only analysis state, scoped to the declared loop and editor lifetime. */
export class RouthSession {
  private selected: number | undefined;
  private saved: SavedRouthAnalysis | undefined;
  private generation = 0;
  private readonly job = new BoundedAnalysisJob();
  private disposed = false;
  constructor(private readonly definition: DeclaredControlAnalysis | (() => DeclaredControlAnalysis)) {}
  private currentDefinition(): DeclaredControlAnalysis {
    return typeof this.definition === "function" ? this.definition() : this.definition;
  }
  async handle(message: unknown, scope: LoopMessageScope, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(message) || !["control-studio.routh.run", "control-studio.routh.cancel", "control-studio.declared-root.run"].includes(String(message.kind)) || message.schemaVersion !== 1
      || message.projectId !== scope.projectId || message.editorSessionId !== scope.editorSessionId
      || message.loopId !== scope.loopId || message.expectedLoopRevision !== scope.loopRevision
      || typeof message.messageId !== "string" || !record(message.payload) || this.disposed) return false;
    if (message.kind === "control-studio.routh.cancel") { this.generation++; this.job.cancel(); await post({ status: "cancelled" }); return true; }
    this.selected = undefined;
    if (message.kind === "control-studio.declared-root.run") return false;
    let generation = 0;
    try {
      const selected = message.payload.selectedValue;
      if (typeof selected !== "number" || !Number.isFinite(selected)) throw new Error("Enter a finite parameter value.");
      generation = ++this.generation;
      const result = await this.job.run<SavedRouthAnalysis["result"]>(path.join(__dirname, "routhWorker.js"), { definition: this.currentDefinition(), selected });
      if (!result || generation !== this.generation || this.disposed) return true;
      this.selected = selected;
      this.saved = { selectedValue: selected, result, definition: structuredClone(this.currentDefinition()) };
      await post({ status: "complete", result });
    } catch (error) { if (generation !== this.generation || this.disposed) return true; await post({ status: "failed", reason: error instanceof Error ? error.message : String(error) }); }
    return true;
  }
  selectedValues(mapping: Readonly<Record<string, string>>): Record<string, number> | undefined {
    const selected = this.selected;
    if (selected === undefined || this.disposed) return undefined;
    const definition = this.currentDefinition();
    return Object.fromEntries(Object.entries(definition.controller.parameters).map(([name, p]) =>
      [mapping[name], name === definition.rootLocus.parameter ? selected : p.value]));
  }
  invalidate(): void { this.generation++; this.job.cancel(); this.selected = undefined; this.saved = undefined; }
  snapshot(): SavedRouthAnalysis | undefined { return this.saved ? structuredClone(this.saved) : undefined; }
  restore(snapshot: unknown): void { if (!isSavedRouthAnalysis(snapshot)) return; if (JSON.stringify(snapshot.definition) !== JSON.stringify(this.currentDefinition())) return; this.saved = structuredClone(snapshot); this.selected = undefined; }
  dispose(): void { this.disposed = true; this.generation++; this.job.cancel(); this.selected = undefined; }
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
