import type { ConnectedIdentificationPorts } from "./connectedIdentification";
import { StudioStudyReset } from "./studioStudyReset";
import { isDeepStrictEqual } from "util";
import type { CommissioningCheckpointWriter } from "./commissioningCheckpoint";
import type { RuntimeVerificationAttempt } from "./verificationClient";
import { RouthSession } from "./routhSession";
import { EngineerWorkflowSession, type PendingManualApplicationOwner } from "./engineerWorkflowSession";
import { acceptedIdentificationModel, selectedModelAnalysis } from "./selectedModelAnalysis";
import { IdentificationWorkflow, type AcceptedIdentificationModel } from "./identification";
import type { DeclaredControlAnalysis } from "./declaredAnalysis";
import { GeneralModelSession, type PeriodicRecordingProvider } from "./generalModelSession";
import { RunValidationSession } from "./runValidationSession";
import { ParameterSession } from "./parameterSession";
import { FiniteSearchSession } from "./finiteSearchSession";
import { connectRuntimeParameters } from "./runtimeParameterConnection";
import { connectRuntimeSearch } from "./runtimeSearchConnection";
import { buildFiniteSearchDraft } from "./finiteSearchDraft";
import type { SavedDeclaredAnalysis } from "./declaredAnalysisPersistence";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMessageScope } from "./messages";
import type { LoopRecording } from "./loopMonitor";
import type { ControlStudioWorkflowResult } from "./workflowResult";
import { identificationOriginalRecording } from "./identificationState";
import { retainedIdentificationSamplesFromRuntimeRecording } from "./experiment";
import { importedRecordingId } from "./recordingIdentity";
import type { IdentificationFormDraft } from "./identificationFormDraft";
import { BoundedAnalysisJob } from "./boundedAnalysisJob";
import { decodeIdentificationState } from "./identificationState";
import type { IdentificationReview } from "./identification";
import * as path from "path";
import { AutomaticStudioFlow } from "./automaticStudioFlow";
import { ReviewedStudioApplicationCoordinator } from "./reviewedStudioApplication";
import { identifyOpenedRecording } from "./openedRecordingIdentification";
import { declaredFeedbackDefinition } from "./declaredFeedbackModel";
import { MimoAnalysisSession, type MimoAnalysisSnapshot } from "./mimoAnalysisSession";
import type { MimoSettledAnalysisOptions, MimoSettledTrial } from "./threePageMimoAnalysis";
import { LeadLagSession } from "./leadLagSession";

/** Owns independent analysis jobs for one editor/loop lifetime. */
export class StudioAnalysisSessions {
  private readonly boundLoop?: ControlLoopDefinition;
  private periodicSourceProvider?: PeriodicRecordingProvider;
  readonly engineer: EngineerWorkflowSession | undefined;
  readonly routh: RouthSession | undefined;
  readonly model = new GeneralModelSession(() => {
    if (this.recordingBusy) throw new Error("Abort the recording analysis before replacing its model.");
    this.engineer?.assertModelReplacementAllowed();
  }, selected => this.boundLoop?.analysis ? declaredFeedbackDefinition(this.boundLoop.analysis, selected) : undefined,
    (recordingId, recordingRevision) => this.periodicSourceProvider?.(recordingId, recordingRevision));
  readonly validation = new RunValidationSession();
  private identificationSession = new IdentificationWorkflow();
  readonly parameters: ParameterSession | undefined;
  readonly search: FiniteSearchSession | undefined;
  readonly mimo = new MimoAnalysisSession();
  readonly leadLag = new LeadLagSession(() => this.model.selectedModel());
  private readonly studyReset = new StudioStudyReset();
  private disposed = false;
  private recordingBusy = false;
  private recordingGeneration = 0;
  private readonly identificationJob = new BoundedAnalysisJob();
  private readonly identificationReplayJob = new BoundedAnalysisJob();
  private identificationGeneration = 0;
  private replayGeneration = 0;
  private readonly automaticFlow: AutomaticStudioFlow;
  private readonly reviewedApplication = new ReviewedStudioApplicationCoordinator();
  get identification(): IdentificationWorkflow { return this.identificationSession; }
  constructor(loop?: ControlLoopDefinition, declared: () => SavedDeclaredAnalysis | undefined = () => undefined,
    retainRecording?: (recording: LoopRecording) => Promise<void>,
    retainPendingManualApplication?: (owner: PendingManualApplicationOwner) => Promise<void>,
    retryManualRecovery?: (post: (payload: Record<string, unknown>) => Promise<void>) => Promise<boolean>,
    retainIsolatedOperation?: (attempt: RuntimeVerificationAttempt) => Promise<void>,
    retainCommissioningCheckpoint?: CommissioningCheckpointWriter,
    periodicSourceProvider?: PeriodicRecordingProvider, liveIdentification?: ConnectedIdentificationPorts) {
    this.boundLoop = loop;
    this.periodicSourceProvider = periodicSourceProvider;
    this.automaticFlow = new AutomaticStudioFlow(
      async (message, scope, post) => {
        const request = message as { kind: string; payload: Record<string, unknown> };
        if (request.kind === "control-studio.engineer.recording" && loop) {
          const generation = ++this.recordingGeneration;
          this.recordingBusy = true; this.cancelIdentification();
          try { await identifyOpenedRecording(this, loop, request.payload.json, this.identificationJob,
            () => !this.disposed && generation === this.recordingGeneration, post, request.payload.recordingSelection); }
          finally { if (generation === this.recordingGeneration) this.recordingBusy = false; }
          return true;
        }
        if (request.kind === "control-studio.engineer.cancel" && this.recordingBusy) {
          this.recordingGeneration++; this.recordingBusy = false; this.cancelIdentification();
        }
        return this.engineer?.handle(message, scope, post) ?? false;
      },
      () => this.engineer?.snapshotStudy()?.task as unknown as Record<string, unknown> | undefined,
    );
    if (!loop) return;
    this.engineer = new EngineerWorkflowSession(loop, () => this.model.selectedModel(), accepted => this.acceptIdentification(accepted, loop), retainRecording,
      retainPendingManualApplication, retryManualRecovery, model => this.model.selectModel(model), retainIsolatedOperation, retainCommissioningCheckpoint, liveIdentification);
    if (loop.analysis) this.routh = new RouthSession(() => this.declaredAnalysis(loop.analysis!));
    this.search = new FiniteSearchSession(() => connectRuntimeSearch(loop), loop.parameterBinding && loop.signals.setpoint
      ? async () => buildFiniteSearchDraft(loop, (await (await connectRuntimeParameters(loop)).client.status()).values) : undefined);
    if (loop.parameterBinding) this.parameters = new ParameterSession(loop.parameterBinding, () => connectRuntimeParameters(loop), () => {
      const routhValues = loop.analysisParameterSymbols && this.routh?.selectedValues(loop.analysisParameterSymbols);
      if (routhValues) return routhValues;
      const analysis = declared();
      if (!analysis || !loop.analysisParameterSymbols) return undefined;
      const definition = analysis.definition;
      return Object.fromEntries(Object.entries(definition.controller.parameters).map(([name, parameter]) =>
        [loop.analysisParameterSymbols![name], name === definition.rootLocus.parameter ? definition.rootLocus.selectedValue : parameter.value]));
    });
  }
  declaredAnalysis(declared: DeclaredControlAnalysis): DeclaredControlAnalysis {
    return selectedModelAnalysis(declared, this.model.selectedModel());
  }
  qualifyIdentification(...input: Parameters<IdentificationWorkflow["qualify"]>) {
    this.engineer?.assertModelReplacementAllowed();
    const qualification = this.identification.qualify(...input);
    this.cancelIdentification();
    this.model.restore(undefined);
    const engineer = this.engineer?.modelChanged();
    this.routh?.invalidate();
    return { qualification, engineer };
  }
  editIdentification(form: IdentificationFormDraft) {
    this.engineer?.assertModelReplacementAllowed();
    this.identification.editForm(form);
    this.cancelIdentification();
    this.model.restore(undefined);
    const engineer = this.engineer?.modelChanged();
    this.routh?.invalidate();
    return { engineer };
  }
  selectIdentificationRecording(recording: LoopRecording, reference?: import("./identificationState").SavedIdentificationInput): Record<string, unknown> | undefined {
    this.engineer?.assertModelReplacementAllowed();
    const original = identificationOriginalRecording(recording, reference?.recording?.kind);
    if (!original || !recording.rawSamples?.length) throw new Error("Select a complete recording first.");
    const saved = this.identification.snapshot();
    const previous = saved?.estimation ?? saved?.current;
    const selected = reference && !saved?.estimation ? new IdentificationWorkflow(reference.maximumNormalizedError) : this.identification;
    const source = selected === this.identification ? previous : reference;
    selected.selectRecording(retainedIdentificationSamplesFromRuntimeRecording(original, recording), source?.excluded ?? [],
      recording.provenance?.source ?? recording.rawSamples[0].source, source?.requestedKind ?? "automatic",
      recording.provenance ? importedRecordingId(recording.provenance) : recording.evidence.recordingId,
      source?.stepResponseSettings, original, source?.firstOrderStepResponseSettings);
    this.cancelIdentification();
    this.identificationSession = selected;
    this.model.restore(undefined);
    this.routh?.invalidate();
    return this.engineer?.modelChanged();
  }
  restoreIdentification(result: Pick<ControlStudioWorkflowResult, "identificationStudy" | "engineerStudy" | "model" | "modelFit" | "generalModelAnalysis">,
    loop?: ControlLoopDefinition): void {
    this.engineer?.assertStudyRestoreAllowed(result.engineerStudy);
    const candidate = new IdentificationWorkflow();
    candidate.restore(result.identificationStudy);
    assertSavedModelAuthority(result, candidate, loop ?? this.boundLoop);
    // Replay and validation happen on an isolated owner. Commit its complete state
    // only after every dependent saved field has matched the same accepted result.
    this.cancelIdentification();
    this.identificationSession = candidate;
  }

  async fitIdentification(): Promise<IdentificationReview | undefined> {
    if (this.disposed) return undefined;
    this.cancelIdentification();
    const generation = this.identificationGeneration, owner = this.identification;
    const review = await owner.fitUsing(input => this.identificationJob.run<IdentificationReview>(
      path.join(__dirname, "identificationReviewWorker.js"), input));
    return this.disposed || generation !== this.identificationGeneration || owner !== this.identification ? undefined : review;
  }

  chooseIdentificationStructure(choice: import("./sensorLagComparison").StructureChoice): IdentificationReview {
    this.engineer?.assertModelReplacementAllowed();
    const review = this.identification.chooseStructure(choice);
    this.cancelIdentification();
    return review;
  }

  analyzeMimo(recording: LoopRecording, recordingId: string, trials: readonly MimoSettledTrial[], options?: MimoSettledAnalysisOptions): MimoAnalysisSnapshot {
    if (!this.boundLoop) throw new Error("A configured loop is required for MIMO analysis.");
    return this.mimo.analyze(this.boundLoop, recording, recordingId, trials, options);
  }

  restoreMimo(recording: LoopRecording | undefined, snapshot: MimoAnalysisSnapshot | undefined): boolean {
    return this.boundLoop ? this.mimo.restore(this.boundLoop, recording, snapshot) : false;
  }

  async restoreIdentificationAsync(result: Parameters<StudioAnalysisSessions["restoreIdentification"]>[0],
    loop?: ControlLoopDefinition): Promise<boolean> {
    const adopt = await this.prepareIdentificationRestoreAsync(result, loop);
    return adopt?.() ?? false;
  }

  async prepareIdentificationRestoreAsync(result: Parameters<StudioAnalysisSessions["restoreIdentification"]>[0],
    loop?: ControlLoopDefinition): Promise<(() => boolean) | undefined> {
    if (this.disposed) return undefined;
    this.engineer?.assertStudyRestoreAllowed(result.engineerStudy);
    if (result.identificationStudy !== undefined && !decodeIdentificationState(result.identificationStudy)) {
      throw new Error("Saved identification inputs are malformed or incompatible.");
    }
    result = structuredClone(result);
    // Recheck an Open separately so a rejected saved result cannot cancel a valid Fit.
    this.identificationReplayJob.cancel();
    const replayGeneration = ++this.replayGeneration;
    const generation = this.identificationGeneration, owner = this.identification;
    const candidate = new IdentificationWorkflow();
    const restored = await candidate.restoreUsing(result.identificationStudy, input => this.identificationReplayJob.run<IdentificationReview>(
      path.join(__dirname, "identificationReviewWorker.js"), input));
    const current = () => !this.disposed && generation === this.identificationGeneration && replayGeneration === this.replayGeneration && owner === this.identification;
    if (!restored || !current()) return undefined;
    this.engineer?.assertStudyRestoreAllowed(result.engineerStudy);
    assertSavedModelAuthority(result, candidate, loop ?? this.boundLoop);
    return () => {
      if (!current()) return false;
      this.engineer?.assertStudyRestoreAllowed(result.engineerStudy);
      this.cancelIdentification();
      this.identificationSession = candidate;
      return true;
    };
  }

  cancelIdentification(): void {
    this.identificationGeneration++; this.replayGeneration++;
    this.identificationJob.cancel(); this.identificationReplayJob.cancel();
  }
  acceptIdentification(accepted: AcceptedIdentificationModel, loop: ControlLoopDefinition): void {
    this.model.selectModel(acceptedIdentificationModel(accepted, selectedModelChannels(loop)));
    this.engineer?.retainIdentificationComparison(accepted);
    this.routh?.invalidate();
  }
  async handle(message: unknown, scope: LoopMessageScope, post: (stage: string, payload: Record<string, unknown>) => Promise<void>, recordingActive = false): Promise<boolean> {
    if (await this.studyReset.handle(message, scope, this, recordingActive || this.recordingBusy || this.automaticFlow.running, post)) return true;
    if (this.engineer && await this.reviewedApplication.handle(message, { engineer: this.engineer, scope,
      isCurrent: () => !this.disposed, post: payload => this.forwardSessionUpdate("engineer", payload, post) })) return true;
    if (await this.automaticFlow.handle(message, scope, (stage, payload) => this.forwardSessionUpdate(stage, payload, post))) return true;
    for (const [stage, session] of [["engineer", this.engineer], ["routh", this.routh], ["general-model", this.model], ["lead-lag", this.leadLag], ["run-validation", this.validation], ["parameters", this.parameters], ["search", this.search]] as const) {
      if (this.disposed) return false;
      if (await session?.handle(message, scope, async payload => {
        await this.forwardSessionUpdate(stage, payload, post);
      })) return true;
    }
    return false;
  }
  private async forwardSessionUpdate(stage: string, payload: Record<string, unknown>, post: (stage: string, payload: Record<string, unknown>) => Promise<void>): Promise<void> {
    if (stage === "engineer" && payload.status === "model-ready" && payload.model) {
      await post("general-model", { status: "imported", model: payload.model });
    }
    let engineerUpdate: Record<string, unknown> | undefined;
    if (stage === "general-model" && payload.status === "imported") {
      this.cancelIdentification();
      this.identification.restore(undefined);
      this.routh?.invalidate();
      engineerUpdate = this.engineer?.modelChanged();
    }
    if (stage === "parameters" && payload.status === "applied" && this.boundLoop?.parameterPurpose === "experiment") {
      // A confirmed project-trial parameter set changes the inputs used by a
      // candidate and its verification. Keep recorded campaigns, but retire
      // dependent design evidence through the existing owner boundary.
      this.routh?.invalidate();
      engineerUpdate = this.engineer?.modelChanged();
    }
    await post(stage, stage === "general-model" ? { ...payload, stageEvidence: this.engineer?.currentStageEvidence() } : payload);
    if (engineerUpdate) await post("engineer", engineerUpdate);
  }
  dispose(): void {
    this.disposed = true;
    this.automaticFlow.dispose();
    this.reviewedApplication.dispose();
    this.cancelIdentification();
    this.engineer?.dispose();
    this.routh?.dispose();
    this.model.dispose(); this.leadLag.dispose(); this.validation.dispose(); this.parameters?.dispose(); this.search?.dispose(); this.mimo.clear();
  }
}

function assertSavedModelAuthority(
  result: Pick<ControlStudioWorkflowResult, "identificationStudy" | "engineerStudy" | "model" | "modelFit" | "generalModelAnalysis">,
  identification: IdentificationWorkflow,
  loop?: ControlLoopDefinition,
): void {
  const selected = result.engineerStudy?.selectedModel, analyzed = result.generalModelAnalysis?.model;
  if (selected && analyzed && !sameJson(selected, analyzed)) {
    throw new Error("Saved selected model and frequency analysis contradict each other.");
  }
  const state = identification.currentState;
  const accepted = state.phase === "accepted" && state.review?.stage === "acceptance_required"
    ? { model: state.review.selected, fit: state.review.fit, evidence: state.review.result.source, review: state.review }
    : undefined;
  if (!accepted) return;
  if (result.model === undefined || result.modelFit === undefined ||
    !sameJson(result.model, accepted.model) || !sameJson(result.modelFit, accepted.fit)) {
    throw new Error("Saved identification dependents contradict the accepted model or fit.");
  }
  if (!loop) return;
  const expected = acceptedIdentificationModel(accepted, selectedModelChannels(loop));
  for (const dependent of [result.engineerStudy?.selectedModel, result.generalModelAnalysis?.model]) {
    if (dependent !== undefined && !sameJson(dependent, expected)) {
      throw new Error("Saved identification dependents contradict the accepted selected model.");
    }
  }
}

function sameJson(left: unknown, right: unknown): boolean {
  return isDeepStrictEqual(left, right);
}

function selectedModelChannels(loop: ControlLoopDefinition): Parameters<typeof acceptedIdentificationModel>[1] {
  const channel = (signal: ControlLoopDefinition["signals"]["processVariable"], id = signal.symbol) => ({
    id, unit: signal.unit, operatingRange: signal.range,
  });
  return {
    inputs: [channel(loop.signals.manipulatedVariable, loop.channels?.inputs[0]?.modelChannelId)],
    outputs: [channel(loop.signals.processVariable, loop.channels?.outputs[0]?.modelChannelId)],
  };
}
