import { decodeGeneralModel, type GeneralModel } from "./generalModel";
import { isSavedGeneralModelAnalysis, isSavedPeriodicFrequencyAnalysis, type SavedGeneralModelAnalysis, type SavedPeriodicFrequencyAnalysis } from "./generalModelPersistence";
import type { LoopMessageScope } from "./messages";
import { isDeepStrictEqual } from "util";
export { isSavedGeneralModelAnalysis, type SavedGeneralModelAnalysis } from "./generalModelPersistence";
export interface PeriodicRecordingSource { input: readonly number[]; output: readonly number[]; sampleTimeSeconds: number; recordingId: string; recordingRevision: string; }
export type PeriodicRecordingProvider = (recordingId: string, recordingRevision: string) => PeriodicRecordingSource | undefined;

/** The plant model a parked loop selected, with the analysis the study saved beside it. The engine calculates every
 * analysis (`controlStudio/analyze`); this owner imports the supplied model and keeps what the study saved. */
export class GeneralModelSession {
  private model: GeneralModel | undefined;
  private saved: SavedGeneralModelAnalysis | undefined;
  private periodic: SavedPeriodicFrequencyAnalysis | undefined;
  constructor(private readonly beforeModelReplacement?: () => void) {}
  async handle(message: unknown, scope: LoopMessageScope, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== scope.projectId || message.editorSessionId !== scope.editorSessionId
      || message.loopId !== scope.loopId || message.expectedLoopRevision !== scope.loopRevision || typeof message.messageId !== "string"
      || !record(message.payload) || message.kind !== "control-studio.general-model.import")
      return false;
    try { this.beforeModelReplacement?.(); }
    catch (error) {
      await post({ status: "failed", reason: error instanceof Error ? error.message : String(error) });
      return true;
    }
    try {
      if (JSON.stringify(message).length > 260000)
        throw new Error("Model message exceeds the 256 kB import limit.");
      const payload = message.payload;
      if (typeof payload.json !== "string" || Buffer.byteLength(payload.json, "utf8") > 256000)
        throw new Error("Choose a model JSON file of at most 256 kB.");
      let parsed: unknown;
      try { parsed = JSON.parse(payload.json); }
      catch { throw new Error("Choose a valid plant model JSON file."); }
      const model = decodeGeneralModel(parsed);
      if (!model)
        throw new Error("Model JSON does not satisfy the supplied model contract.");
      this.selectModel(model);
      this.saved = undefined; this.periodic = undefined;
      await post({ status: "imported", model });
    }
    catch (error) {
      await post({ status: "failed", reason: error instanceof Error ? error.message : String(error) });
    }
    return true;
  }
  snapshot(): SavedGeneralModelAnalysis | undefined {
    return this.saved ? JSON.parse(JSON.stringify(this.saved)) : undefined;
  }
  periodicSnapshot(): SavedPeriodicFrequencyAnalysis | undefined {
    return this.periodic ? JSON.parse(JSON.stringify(this.periodic)) : undefined;
  }
  restorePeriodic(value: SavedPeriodicFrequencyAnalysis | undefined): void {
    if (value && !isSavedPeriodicFrequencyAnalysis(value)) throw new Error("Saved periodic frequency analysis is invalid.");
    this.periodic = value ? JSON.parse(JSON.stringify(value)) : undefined;
    if (this.saved && this.periodic) this.saved = { ...this.saved, periodic: this.periodic };
  }
  selectedModel(): GeneralModel | undefined {
    return this.model ? JSON.parse(JSON.stringify(this.model)) : undefined;
  }
  selectModel(value: GeneralModel): void {
    const model = decodeGeneralModel(value);
    if (!model) throw new Error("Selected model does not satisfy the shared model contract.");
    if (isDeepStrictEqual(this.model, model)) return;
    this.model = model; this.saved = undefined; this.periodic = undefined;
  }
  restore(saved: SavedGeneralModelAnalysis | undefined): void {
    if (saved && !isSavedGeneralModelAnalysis(saved)) throw new Error("Saved model analysis is invalid.");
    this.saved = saved ? JSON.parse(JSON.stringify(saved)) : undefined;
    this.periodic = this.saved?.periodic;
    this.model = this.saved?.model;
  }
  dispose(): void {
    this.model = undefined; this.saved = undefined; this.periodic = undefined;
  }
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
