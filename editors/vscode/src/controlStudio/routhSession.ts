import type { DeclaredControlAnalysis } from "./declaredAnalysis";
import { isSavedRouthAnalysis, type SavedRouthAnalysis } from "./routhPersistence";
export { isSavedRouthAnalysis, type SavedRouthAnalysis } from "./routhPersistence";

/** Keeps the Routh reading a parked loop saved: read-only, and only for the declared definition it was read from. The
 * engine calculates Routh readings (`analysis/routh.rs`), so this owner calculates nothing. */
export class RouthSession {
  private saved: SavedRouthAnalysis | undefined;
  constructor(private readonly definition: DeclaredControlAnalysis | (() => DeclaredControlAnalysis)) {}
  private currentDefinition(): DeclaredControlAnalysis {
    return typeof this.definition === "function" ? this.definition() : this.definition;
  }
  invalidate(): void { this.saved = undefined; }
  snapshot(): SavedRouthAnalysis | undefined { return this.saved ? structuredClone(this.saved) : undefined; }
  restore(snapshot: unknown): void {
    if (!isSavedRouthAnalysis(snapshot)) return;
    if (JSON.stringify(snapshot.definition) !== JSON.stringify(this.currentDefinition())) return;
    this.saved = structuredClone(snapshot);
  }
}
