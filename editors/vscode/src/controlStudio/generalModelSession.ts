import { decodeGeneralFeedback, type GeneralFeedbackDefinition } from "./generalFeedback";
import type { SavedGeneralFeedback } from "./generalFeedbackPersistence";
import { BoundedAnalysisJob } from "./boundedAnalysisJob";
import * as path from "path";
import { isDeepStrictEqual } from "util";
import type { LoopMessageScope } from "./messages";
import { decodeGeneralModel, type GeneralModel, type GeneralModelFrequencyResult } from "./generalModel";
import { isSavedGeneralModelAnalysis, isSavedPeriodicFrequencyAnalysis, validModelGrid, type SavedGeneralModelAnalysis, type SavedPeriodicFrequencyAnalysis } from "./generalModelPersistence";
import { estimatePeriodicFrequency } from "./threePageAnalysisOperations";
import type { ModelTimeResponse } from "./generalModelTime";
import type { ModelPoleView } from "./generalModelPoleView";
import { RECORDING_SAMPLE_LIMIT } from "./controlStudioLimits";
export { isSavedGeneralModelAnalysis, type SavedGeneralModelAnalysis } from "./generalModelPersistence";
export type DeclaredFeedbackProvider = (model: GeneralModel) => GeneralFeedbackDefinition | undefined;
export interface PeriodicRecordingSource { input: readonly number[]; output: readonly number[]; sampleTimeSeconds: number; recordingId: string; recordingRevision: string; }
export type PeriodicRecordingProvider = (recordingId: string, recordingRevision: string) => PeriodicRecordingSource | undefined;
export class GeneralModelSession {
  private model: GeneralModel | undefined;
  private controller: GeneralModel | undefined;
  private saved: SavedGeneralModelAnalysis | undefined;
  private periodic: SavedPeriodicFrequencyAnalysis | undefined;
  private generation = 0;
  private readonly job = new BoundedAnalysisJob();
  constructor(
    private readonly beforeModelReplacement?: () => void,
    private readonly declaredFeedbackProvider?: DeclaredFeedbackProvider,
    private readonly periodicRecordingProvider?: PeriodicRecordingProvider,
  ) {}
  async handle(message: unknown, scope: LoopMessageScope, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== scope.projectId || message.editorSessionId !== scope.editorSessionId
      || message.loopId !== scope.loopId || message.expectedLoopRevision !== scope.loopRevision || typeof message.messageId !== "string"
      || !record(message.payload) || typeof message.kind !== "string" || !message.kind.startsWith("control-studio.general-model."))
      return false;
    const action = message.kind.slice("control-studio.general-model.".length);
    const payload = message.payload;
    if (!["import", "import-controller", "clear-controller", "invalidate", "analyze", "response", "pole-view", "periodic", "cancel"].includes(action))
      return false;
    if (action === "import") {
      try { this.beforeModelReplacement?.(); }
      catch (error) {
        await post({ status: "failed", reason: error instanceof Error ? error.message : String(error) });
        return true;
      }
    }
    try {
      if (JSON.stringify(message).length > 260000)
        throw new Error("Model message exceeds the 256 kB import limit.");
      if (action === "invalidate" || action === "clear-controller") {
        this.cancel(); this.saved = undefined; this.periodic = undefined;
        if (action === "clear-controller") this.controller = undefined;
        await post({ status: action === "clear-controller" ? "controller-cleared" : "edited", reason: "Analysis inputs changed. Calculate again before saving." }); return true;
      }
      if (action === "cancel") {
        this.cancel();
        await post({ status: "cancelled", reason: "Model analysis cancelled." });
        return true;
      }
      if (action === "periodic") {
        const recordingId = typeof payload.recordingId === "string" ? payload.recordingId : undefined;
        const recordingRevision = typeof payload.recordingRevision === "string" ? payload.recordingRevision : undefined;
        const source = recordingId && recordingRevision ? this.periodicRecordingProvider?.(recordingId, recordingRevision) : undefined;
        if (!source || source.recordingId !== recordingId || source.recordingRevision !== recordingRevision || source.input.length !== source.output.length || source.input.length > RECORDING_SAMPLE_LIMIT
            || !source.input.every(finite) || !source.output.every(finite)
            || !Number.isSafeInteger(payload.startupSamples) || (payload.startupSamples as number) < 1
            || !Array.isArray(payload.frequencies) || !payload.frequencies.every(finite) || payload.frequencies.length === 0
            || !Number.isSafeInteger(payload.segmentCount) || (payload.segmentCount as number) < 2
            || !Number.isFinite(payload.minimumCoherence) || (payload.minimumCoherence as number) < 0 || (payload.minimumCoherence as number) > 1
            ) throw new Error("Periodic analysis requires a complete retained recording, startup discard and bounded frequency settings.");
        const result = estimatePeriodicFrequency(source.input, source.output, source.sampleTimeSeconds, payload.frequencies as number[], {
          segmentCount: payload.segmentCount as number, minimumCoherence: payload.minimumCoherence as number, startupSamples: payload.startupSamples as number,
        });
        if (result.status !== "qualified") throw new Error(`Periodic frequency response was rejected: ${result.reason}.`);
        const periodic: SavedPeriodicFrequencyAnalysis = { startupSamples: payload.startupSamples as number, sampleTimeSeconds: source.sampleTimeSeconds,
          frequencies: payload.frequencies as number[], segmentCount: payload.segmentCount as number, minimumCoherence: payload.minimumCoherence as number,
          recordingId: source.recordingId, recordingRevision: source.recordingRevision, result };
        if (!isSavedPeriodicFrequencyAnalysis(periodic)) throw new Error("Periodic frequency response did not satisfy the saved analysis contract.");
        this.periodic = periodic;
        if (this.saved) this.saved = { ...this.saved, periodic };
        await post({ status: "periodic-complete", periodic });
        return true;
      }
      if (action === "import" || action === "import-controller") {
        if (typeof payload.json !== "string" || Buffer.byteLength(payload.json, "utf8") > 256000)
          throw new Error("Choose a model JSON file of at most 256 kB.");
        let parsed: unknown;
        try { parsed = JSON.parse(payload.json); }
        catch { throw new Error("Choose a valid plant model JSON file."); }
        const model = decodeGeneralModel(parsed);
        if (!model)
          throw new Error("Model JSON does not satisfy the supplied model contract.");
        this.cancel();
        if (action === "import") this.selectModel(model); else this.controller = model;
        this.saved = undefined; this.periodic = undefined;
        await post({ status: action === "import" ? "imported" : "controller-imported", model });
        return true;
      }
      const model = this.model;
      if (!model)
        throw new Error("Import a model first.");
      const feedback = this.controller ? decodeGeneralFeedback({ plant: model, controller: this.controller, feedback: "negative",
        plantChannel: { inputId: payload.plantInputId ?? model.inputs[0]?.id, outputId: payload.plantOutputId ?? model.outputs[0]?.id },
        controllerChannel: { inputId: payload.controllerInputId ?? this.controller.inputs[0]?.id, outputId: payload.controllerOutputId ?? this.controller.outputs[0]?.id } })
        : this.declaredFeedbackProvider?.(model);
      if (this.controller && !feedback) throw new Error("Select compatible plant/controller channels with inverse units and the same model domain/sample time.");
      const feedbackController = feedback?.controller;
      const workChannels = model.inputs.length * model.outputs.length * (feedback ? 2 : 1)
        + (feedbackController ? feedbackController.inputs.length * feedbackController.outputs.length : 0);
      const maximumFrequency = model.kind === "discrete-state-space" ? Math.min(10, Math.PI / model.sampleTimeSeconds! * .5) : 10;
      const requestedGrid = action === "response" || action === "pole-view" ? this.saved?.grid ?? { minimumFrequency: Math.min(.1, maximumFrequency / 10), maximumFrequency, points: 32 } : payload;
      if (!validModelGrid(requestedGrid, workChannels))
        throw new Error("Choose 2–512 points, a positive increasing range, and at most 16384 channel-frequency points.");
      this.cancel();
      const generation = this.generation;
      const previous = this.saved;
      const grid = { minimumFrequency: requestedGrid.minimumFrequency, maximumFrequency: requestedGrid.maximumFrequency, points: requestedGrid.points };
      await post({ status: "running", reason: "Calculating model channels in a bounded worker…" });
      if (generation !== this.generation) return true;
      const jobResult = await this.job.run<{ result: GeneralModelFrequencyResult; feedback?: SavedGeneralFeedback; response?: ModelTimeResponse; poleView?: ModelPoleView }>(path.join(__dirname, "generalModelWorker.js"), {
        model, grid, feedback, ...(action === "response" ? { response: payload } : {}), ...(action === "pole-view" ? { poleView: payload.view } : {}),
      });
      const result = jobResult?.result;
      if (generation !== this.generation || !result)
        return true;
      if (result.status !== "qualified")
        throw new Error(result.reason);
      const saved = { ...previous, model, grid, result, ...(jobResult?.feedback ? { feedback: jobResult.feedback } : {}),
        ...(jobResult?.response ? { response: jobResult.response } : {}), ...(jobResult?.poleView ? { poleView: jobResult.poleView } : {}), ...(this.periodic ? { periodic: this.periodic } : {}) };
      if (!isSavedGeneralModelAnalysis(saved))
        throw new Error("Worker returned invalid model evidence.");
      this.saved = saved;
      await post({ status: "complete", saved });
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
    this.cancel(); this.model = model; this.saved = undefined; this.periodic = undefined;
  }
  restore(saved: SavedGeneralModelAnalysis | undefined): void {
    if (saved && !isSavedGeneralModelAnalysis(saved)) throw new Error("Saved model analysis is invalid.");
    const restored = saved ? JSON.parse(JSON.stringify(saved)) : undefined;
    this.cancel();
    this.saved = restored;
    this.periodic = restored?.periodic;
    this.model = this.saved?.model;
    this.controller = this.saved?.feedback?.definition.controller;
  }
  dispose(): void {
    this.cancel();
    this.model = undefined;
    this.controller = undefined;
    this.saved = undefined;
    this.periodic = undefined;
  }
  private cancel(): void { this.generation++; this.job.cancel(); }
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
