import { experimentFromSamples } from "./identificationExperiment";
import { controllerChannelMapping, controllerRequirementLoop, ChannelRequirementsRequired } from "./controllerChannelPreferences";
import { reconcileManualApplication } from "./manualApplicationReconciliation";
import { engineerPrediction, selectedModelFrequencyAnalysis } from "./engineerPrediction";
import { decodeModelComparison, type ModelComparison } from "./modelComparison";
import { acquisitionSettings, configuredAcquisition, AcquisitionLimitsRequired } from "./acquisitionSettings";
import { decodeRecordingCatalog, type RecordingSignalCatalog } from "./openedRecordingChannels";
import { controllerTuningPreferences } from "./controllerTuningPreferences";
import type { CommissioningCheckpoint, CommissioningCheckpointWriter } from "./commissioningCheckpoint";
import { stateControllerPoles, type StateControllerDesign } from "./stateControllerDesign";
import { isControllerFamily } from "./controllerFamilies";
import { bindRuntimeConnection } from "./runtimeConnection";
import { tuneControllerAdapter, proportionalDesignLoop } from "./controllerTuning";
import { validEngineerScenarios } from "./engineerVerificationCampaign";
import { currentControllerGains, decodeControllerSettingsPins, pinnedControllerGains } from "./controllerSettingsPins";
import { controllerAdapter, type ControllerGains } from "./controllerAdapters";
import { modelReadyReason } from "./engineerWorkflowPresentation";
import { designManualProportionalCandidate } from "./manualProportionalDesign";
import { type CommissioningResult } from "./proportionalCommissioning";
import type { LoopRecording } from "./loopMonitor";
import type { EngineerGainPin } from "./engineerGainChoice";
import { decodePidLaw } from "./controllerLaw";
import { selectedModelRelayCycle, type RelayCycleEvidence } from "./relayCycleEvidence";
import { decodeEngineerStudy, type EngineerStudy, type SavedEngineerFamilyEvaluation } from "./engineerStudyPersistence";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import type { ControlStudioStageEvidence } from "./stageAvailability";
import type { LoopMessageScope } from "./messages";
import { engineerRequirements, proportionalParameter, type EngineerRequirements } from "./engineerVerificationPlan";
import { connectRuntimeSearch } from "./runtimeSearchConnection";
import type { SearchConnection } from "./finiteSearchSession";
import { compareOnModel } from "./modelPredictionRuns";
import { connectRuntimeParameters } from "./runtimeParameterConnection";
import { RuntimeVerificationClient, type RuntimeVerificationResult, type RuntimeVerificationAttempt } from "./verificationClient";
import { type ParameterStatus } from "./parameterApplication";
import { isDeepStrictEqual } from "util";
import { type ConnectedIdentificationPorts } from "./connectedIdentification";
import { identificationRecovery, type IdentificationRecovery } from "./connectedIdentificationRecovery";
import type { AutomaticIdentificationResult } from "./automaticIdentification";
import { randomUUID } from "crypto";
import { previewAutomaticRequirements } from "./automaticEngineerRequirements";
import { identifiedModelPresentation, type AcceptedIdentificationModel } from "./identification";
import { bindingRuntime, classifyMainWorkflowFailure, MainWorkflowError, nextActionSentence, presentMainWorkflowFailure, qualifyModel,
  type MainWorkflowClassification } from "./mainWorkflowFailure";
import { ApplyPrecondition } from "./commissioningStartingPoint";
import { AutomaticControllerSelectionError } from "./automaticControllerRecommendation";
import { assertEngineerModelCompatibility, assertQualifiedModelResult } from "./engineerModelCompatibility";
import { createEngineerSessionState, type EngineerSessionState } from "./engineerSessionState";
import { candidateMargins } from "./candidateMargins";
import type { EngineerActionContext, EngineerActionHost } from "./engineerActions/host";
import { acceptIdentificationResult, identifyPlant } from "./engineerActions/find";
import { tuneCandidate } from "./engineerActions/tune";
import { validateCandidate } from "./engineerActions/verify";
import { confirmReviewedOperation, reviewApplication } from "./engineerActions/application";
import { reattachApplyObservation, reviewInterruptedRollback, reviewReplacementRuntime, retireInterruptedOperation } from "./engineerActions/recovery";
import { observationInProgress, type ObservedCheckpoint } from "./applyObservation";
import { previewManualSettings as previewSettings } from "./engineerActions/manualPreview";

/** Serializable host owner; deliberately excludes the single-use review and runtime client. */
export interface PendingManualApplicationOwner {
  operation: "apply" | "rollback";
  origin: { program_revision: string; binding_revision: string;
    review?: { transaction_id?: string; active_revision: string; previous_revision: string | null; values: Readonly<Record<string, number>>; requested: Readonly<Record<string, number>> } };
}

export interface PendingManualApplicationRecovery extends PendingManualApplicationOwner {
  observed: ParameterStatus;
}

export interface UnresolvedManualApplicationRecovery {
  operation: "apply" | "rollback";
  origin: PendingManualApplicationOwner["origin"];
  reason: string;
}

export type ManualApplicationCloseRecovery = PendingManualApplicationRecovery | UnresolvedManualApplicationRecovery;

/** Connects the existing model, design, isolated runtime and parameter transaction services. */
export class EngineerWorkflowSession {
  private readonly state: EngineerSessionState;
  /** The reattaching or recovering identification this session holds; closing releases its clients only (A8.6, A10.4). */
  private identificationRecovery: IdentificationRecovery | undefined;
  /** Closing the panel detaches a reattached Apply observation: the runtime keeps recording it, and nothing is aborted (A10.4). */
  private observationDetached = false;
  private readonly host: EngineerActionHost;
  /** Whether the engine serves the loop (set from the panel's open answer): a served loop's Controller list is the engine's, so this session posts none. */
  private engineServed = false;
  private get loop(): ControlLoopDefinition { return controllerRequirementLoop(this.configuredLoop, this.state.channelMapping, this.state.task); }
  constructor(private readonly configuredLoop: ControlLoopDefinition, private readonly model: () => GeneralModel | undefined,
    private readonly acceptIdentification: (accepted: AcceptedIdentificationModel) => void,
    private readonly retainRecording?: (recording: LoopRecording) => Promise<void>,
    private readonly retainPendingManualApplication?: (owner: PendingManualApplicationOwner) => Promise<void>,
    private readonly retryManualRecovery?: (post: (payload: Record<string, unknown>) => Promise<void>) => Promise<boolean>,
    private readonly acceptFittedModel?: (model: GeneralModel) => void,
    private readonly retainIsolatedOperation?: (attempt: RuntimeVerificationAttempt) => Promise<void>,
    private readonly retainCommissioningCheckpoint?: CommissioningCheckpointWriter,
    private readonly liveIdentification?: ConnectedIdentificationPorts) {
    this.state = createEngineerSessionState(configuredLoop.verificationScenarios);
    this.host = this.actionHost();
  }

  /** The one view the action owners (`engineerActions/*`) have of this session. */
  private actionHost(): EngineerActionHost {
    return {
      state: this.state, loop: () => this.loop, model: () => this.model(),
      liveIdentification: this.liveIdentification, retainIsolatedOperation: this.retainIsolatedOperation, acceptFittedModel: this.acceptFittedModel,
      retainRecording: this.retainRecording, retainPendingManualApplication: this.retainPendingManualApplication,
      retainCommissioningCheckpoint: this.retainCommissioningCheckpoint,
      acceptIdentification: accepted => this.acceptIdentification(accepted),
      retainIdentificationComparison: accepted => this.retainIdentificationComparison(accepted),
      retainAttempt: attempt => this.retainAttempt(attempt),
      updateFamilyEvaluation: (family, status, reason) => this.updateFamilyEvaluation(family, status, reason),
      clearIncompatiblePin: () => this.clearIncompatiblePin(), compatiblePin: pin => this.compatiblePin(pin),
      relayCycle: () => this.relayCycle(), currentStageEvidence: () => this.currentStageEvidence(),
      availableFamilies: () => this.availableFamilies(), requirementLoop: () => this.requirementLoop(), scenarioLoop: () => this.scenarioLoop(),
      inputSignature: () => this.inputSignature(), checkVerifiedProject: () => this.checkVerifiedProject(), invalidate: () => this.invalidate(),
      requireManualRecovery: (recovery, reason) => this.requireManualRecovery(recovery, reason),
      compareOnModel: (connection, model, task, observed, values, generation) => this.compareOnModel(connection, model, task, observed, values, generation),
      programLawPrediction: (model, task, values, generation, respond) => this.programLawPrediction(model, task, values, generation, respond),
      runningControllerGains: () => this.runningControllerGains(),
    };
  }

  retainIdentificationComparison(accepted: AcceptedIdentificationModel): void {
    const selected = this.model();
    if (!selected) return;
    this.state.modelComparison = { modelRevision: selected.revision,
      presentation: { ...identifiedModelPresentation(accepted.model, accepted.fit,
        this.loop.signals.manipulatedVariable.unit, this.loop.signals.processVariable.unit),
      experiment: experimentFromSamples(this.loop, accepted.review.validation) } };
  }

  retainOpenedRecordings(runs: readonly RuntimeVerificationResult[]): void {
    this.assertModelReplacementAllowed();
    const id = `opened-recording:${runs.map(result => result.run.id).join(":")}`;
    if (this.state.campaigns.some(campaign => campaign.id === id)) return;
    const campaigns = [...this.state.campaigns, { id, status: "completed" as const, runs }];
    if (!decodeEngineerStudy({ campaigns })) throw new Error("The recording archive exceeds the study limits or contains invalid data.");
    this.state.campaigns.push(campaigns[campaigns.length - 1]);
  }

  retainRecordingCatalog(value: RecordingSignalCatalog): void {
    const catalog = decodeRecordingCatalog(value);
    if (!catalog) throw new Error("The recording signal selection is invalid.");
    this.state.recordingCatalog = catalog;
  }

  retainModelComparison(value: ModelComparison): void {
    const comparison = decodeModelComparison(value, this.model());
    if (!comparison) throw new Error("The recorded fit does not match the selected model.");
    this.state.modelComparison = comparison;
  }

  modelChanged(): Record<string, unknown> {
    this.state.modelValidation = undefined;
    this.invalidate();
    this.clearIncompatiblePin();
    let model: GeneralModel | undefined;
    let modelAnalysis: ReturnType<typeof selectedModelFrequencyAnalysis>;
    try {
      model = this.model();
      if (!model) {
        this.state.modelComparison = undefined;
        return { status: "edited", stageEvidence: this.currentStageEvidence() };
      }
      this.state.modelComparison = decodeModelComparison(this.state.modelComparison, model);
      modelAnalysis = selectedModelFrequencyAnalysis(model);
      qualifyModel(() => assertQualifiedModelResult(this.loop, model!));
      this.state.task = previewAutomaticRequirements(this.requirementLoop(), model, this.state.task, this.state.campaigns);
      const evidence = this.currentStageEvidence(), families = this.availableFamilies(), relayCycle = this.relayCycle();
      return { status: "model-ready", task: this.state.task, compatibleControllerRequired: evidence?.hasModel !== true, model, ...(this.state.modelComparison ? { identificationPresentation: this.state.modelComparison.presentation } : {}), ...(modelAnalysis ? { modelAnalysis } : {}), ...(relayCycle ? { relayCycle } : {}), stageEvidence: this.currentStageEvidence(), gainChoice: this.state.gainChoice,
        controllerChoice: this.state.controllerChoice, ...(this.engineServed ? {} : { availableFamilies: families, recommendedFamily: this.recommendedFamily() }),
        reason: evidence?.hasModel ? modelReadyReason(model, families) : "Choose a compatible controller on Tune." };
    } catch (error) {
      const failure = failureOf(error);
      return { status: "unavailable", failure: presentMainWorkflowFailure(explanation(error), failure), ...(model ? { model } : {}), ...(modelAnalysis ? { modelAnalysis } : {}), stageEvidence: this.currentStageEvidence(),
        reason: `${explanation(error)}${nextActionSentence(failure.nextAction)}` };
    }
  }

  /** Manual preview (B4.5): read-only, and answered by panel.ts as stage "tune" for the latest request only. */
  previewManualSettings(settings: Readonly<Record<string, number>>): Promise<Record<string, unknown>> {
    return previewSettings(this.host, settings);
  }

  async handle(message: unknown, scope: LoopMessageScope, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== scope.projectId || message.editorSessionId !== scope.editorSessionId
      || message.loopId !== scope.loopId || message.expectedLoopRevision !== scope.loopRevision || !record(message.payload)
      || typeof message.messageId !== "string" || typeof message.kind !== "string" || !message.kind.startsWith("control-studio.engineer.")) return false;
    if (this.state.handledMessages.has(message.messageId)) {
      await post({ status: "busy", reason: "This request has already been handled; no runtime operation was repeated." });
      return true;
    }
    if (this.state.handledMessages.size >= 4096) {
      await post({ status: "busy", reason: "Session request limit reached. Finish recovery and reopen the panel before starting new work." });
      return true;
    }
    this.state.handledMessages.add(message.messageId);
    const action = message.kind.slice("control-studio.engineer.".length);
    const requestedApplicationMode = message.payload.applicationMode;
    if (requestedApplicationMode !== undefined && requestedApplicationMode !== "auto" && requestedApplicationMode !== "manual") return false;
    const applicationMode = requestedApplicationMode ?? this.state.applicationMode;
    if (!["find", "tune", "verify", "apply", "confirm", "rollback", "invalidate", "cancel", "clear-review", "retry-recovery", "inspect", "review-runtime"].includes(action)) return false;
    if (action === "inspect") {
      const vscode = await import("vscode");
      await vscode.commands.executeCommand("workbench.action.quickOpen", `#${this.loop.controller.instance}`);
      return true;
    }
    if (this.state.interruptedCheckpoint && !["completed", "recovered", "retired"].includes(this.state.interruptedCheckpoint.stage) && !["rollback", "confirm", "cancel", "review-runtime"].includes(action)) {
      await post({ status: "recovery-required", commissioningCheckpoint: true, reason: "Commissioning was interrupted. Review rollback to reconcile the retained runtime operations before starting new work." });
      return true;
    }
    if (!this.state.busy && this.state.commissioned?.recoveryConfirmed === false && !["rollback", "confirm", "review-runtime"].includes(action)) {
      await post({ status: "recovery-required", commissioningCheckpoint: true, commissioning: this.state.commissioned,
        reason: "Recovery remains unconfirmed. Review Rollback on the current runtime before starting new work." });
      return true;
    }
    if (action === "cancel" && this.state.applySettling) { this.state.applySettling.cancelled = true; return true; }
    if (this.state.busy === "apply" && action === "cancel" && this.state.commissioning) {
      const generation = this.state.generation;
      const result = await this.state.commissioning.cancel();
      if (result && generation === this.state.generation) {
        this.state.commissioned = result;
        if (result.recording) await this.retainRecording?.(result.recording);
        if (result.recoveryConfirmed) this.state.busy = undefined;
        await post({ status: result.status === "passed" ? "commissioned" : "failed", commissioning: result,
          observed: result.parameters, reason: `Auto commissioning ${result.status}: ${result.reason}` });
      }
      return true;
    }
    if (this.state.busy === "apply" && action !== "review-runtime") { await post({ status: "busy", reason: "The parameter transaction is awaiting observed confirmation." }); return true; }
    if (action === "invalidate" || action === "cancel") {
      const current = this.invalidateInputs(action, message.payload, applicationMode, post);
      const operation = Promise.all([this.state.pendingInputOperation, current]).then(() => true);
      this.state.pendingInputOperation = operation;
      try { return await operation; }
      finally { if (this.state.pendingInputOperation === operation) this.state.pendingInputOperation = undefined; }
    }
    // Input changes must finish cancellation and their replies before a new
    // candidate is produced; a late edit reply would otherwise hide that result.
    const generation = this.state.generation;
    await this.state.pendingInputOperation;
    if (generation !== this.state.generation) return true;
    if (this.state.busy && action !== "review-runtime") { await post({ status: "busy", reason: "Verification is running. Cancel it before changing the candidate." }); return true; }
    if (this.state.manualApplicationRecovery && action !== "apply" && action !== "rollback" && action !== "retry-recovery") {
      await post({ status: "recovery-required", retryManualRecovery: true, observed: "observed" in this.state.manualApplicationRecovery.recovery
        ? this.state.manualApplicationRecovery.recovery.observed : undefined, reason: this.state.manualApplicationRecovery.reason });
      return true;
    }
    if (applicationMode === "auto" || applicationMode === "manual") this.state.applicationMode = applicationMode;
    if (action === "clear-review") { this.state.review = undefined; await post({ status: "review-cleared", reason: "Apply will review the selected application mode." }); return true; }
    const respond = async (payload: Record<string, unknown>) => {
      if (generation !== this.state.generation) return;
      if (payload.status === "model-ready" && this.model()) {
        payload.modelAnalysis = selectedModelFrequencyAnalysis(this.model()!); this.state.task = previewAutomaticRequirements(this.requirementLoop(), this.model()!, this.state.task, this.state.campaigns);
      }
      if (payload.status === "candidate" && this.state.design) {
        const gains = pinnedControllerGains(this.loop, this.state.family, this.state.design.candidate, this.state.settingsPins);
        if (this.state.settingsPins && this.state.task && !this.loop.controller.stateFeedback && !this.loop.controller.onOff && !this.loop.controller.project && !decodePidLaw(this.loop.controller.law, this.loop.parameterBinding)) {
          const parameter = proportionalParameter(this.loop);
          this.state.design = designManualProportionalCandidate(this.model(), gains.proportionalGain!, {
            referenceChange: this.state.task.referenceChange, constantInputDisturbance: this.state.task.constantInputDisturbance ?? 0,
            maximumSteadyError: this.state.task.maximumError - this.state.task.settlingBand, settlingBand: this.state.task.settlingBand,
            settlingTimeSeconds: this.state.task.settlingTimeSeconds,
            proportionalGainBounds: { minimum: parameter.minimum, maximum: parameter.maximum },
          });
        }
        if (this.state.settingsPins) {
          delete payload.stability; delete payload.attempts; this.state.familyEvaluations = undefined;
          payload.reason = "Pinned settings retained in the candidate. Verify the actual ST before application.";
        }
        this.state.design = { ...this.state.design, candidate: { ...this.state.design.candidate, ...gains } } as typeof this.state.design;
        this.state.values = controllerAdapter(this.loop).materialize(this.state.design!.candidate, this.state.values ?? {});
        if (this.loop.controller.stateFeedback) this.state.design = { ...this.state.design as StateControllerDesign,
          ...(this.state.settingsPins ? { status: "exploratory" as const, scope: "Manual coefficient pins retained; current sampled poles are recalculated, and no original optimality claim is retained." } : {}),
          closedLoopPoles: stateControllerPoles(this.loop, this.model()!, this.state.values) };
        payload.design = this.state.design; payload.values = this.state.values;
        this.state.retainedDraft = { modelRevision: this.model()!.revision, loopRevision: this.loop.revision, family: this.state.family,
          values: structuredClone(this.state.values), design: structuredClone(this.state.design) as unknown as Record<string, unknown> };
      }
      if (payload.status === "model-ready" && this.model()) {
        payload.modelAnalysis = selectedModelFrequencyAnalysis(this.model()!); this.state.task = previewAutomaticRequirements(this.requirementLoop(), this.model()!, this.state.task, this.state.campaigns);
      }
      if (payload.status === "candidate" && this.state.design) {
        const plots = engineerPrediction(this.model(), this.loop, this.state.design.candidate, this.state.task, await this.runningControllerGains());
        payload.plots = plots;
        // A program's own law keeps its ST prediction on the chart; its linear loop still gives the margins (Specification 35 §17.10).
        const margins = candidateMargins(this.model(), this.loop, this.state.design, this.state.task, plots);
        if (margins) payload.robustness = margins;
      }
      if (generation === this.state.generation) await post({ ...payload, ...(this.state.task ? { task: this.state.task } : {}), ...(this.state.modelValidation && payload.status !== "running" ? { modelValidation: this.state.modelValidation } : {}), requiredScenarios: this.state.requiredScenarios, channelMapping: this.state.channelMapping, ...(this.state.commissioned?.recoveryConfirmed === false ? { commissioningCheckpoint: true } : {}), tuningPreferences: this.state.tuningPreferences, acquisitionSettings: this.state.acquisitionPreferences, scenarioDefinitions: this.state.scenarioDefinitions ?? [], stageEvidence: this.currentStageEvidence(), gainChoice: this.state.gainChoice, settingsPins: this.state.settingsPins ?? null, verificationAttempts: this.state.verificationAttempts, controllerChoice: this.state.controllerChoice, ...(this.engineServed ? {} : { availableFamilies: this.availableFamilies(), recommendedFamily: this.recommendedFamily() }), studyAvailable: Boolean(this.model() || this.state.task || this.state.campaigns.length) }); };
    const context: EngineerActionContext = { payload: message.payload, respond, post, generation, applicationMode: applicationMode as "auto" | "manual" };
    try {
      if (action === "retry-recovery") {
        if (!this.retryManualRecovery) throw new Error("No persisted Manual recovery is available to retry.");
        await this.retryManualRecovery(async payload => respond({ ...payload, retryManualRecovery: true }));
        return true;
      }
      if (action === "find") return await identifyPlant(this.host, context);
      if (action === "tune") return await tuneCandidate(this.host, context);

      if (action === "verify") return await validateCandidate(this.host, context);
      if (action === "review-runtime") return await reviewReplacementRuntime(this.host, context);
      if (action === "confirm" && this.state.runtimeReplacementReview && this.state.interruptedCheckpoint) return await retireInterruptedOperation(this.host, context);
      if (action === "rollback" && this.state.interruptedCheckpoint) return await reviewInterruptedRollback(this.host, context);
      if (action === "confirm") return await confirmReviewedOperation(this.host, context);
      return await reviewApplication(this.host, context, action as "apply" | "rollback");
    } catch (error) {
      this.state.review = undefined;
      if (this.state.manualApplicationRecovery) {
        const owner = this.state.manualApplicationRecovery.recovery;
        const reason = this.requireManualRecovery(owner,
          `The retained ${owner.operation} is still unconfirmed: ${explanation(error)}. Retry recovery after reconnecting.`);
        await respond({ status: "recovery-required", retryManualRecovery: true, reason });
        return true;
      }
      if (action === "tune") { this.state.design = undefined; this.state.values = undefined; this.state.verified = undefined; this.state.campaignPassed = false; }
      const recovery = this.state.busy === "apply" ? this.state.commissioning?.recordedOutcome() : undefined;
      if (recovery?.recoveryConfirmed === false) this.state.commissioned = recovery;
      if (error instanceof AutomaticControllerSelectionError) {
        this.state.familyEvaluations = error.attempts.map(attempt => ({ family: attempt.structure,
          status: "unavailable", reason: attempt.reason }));
      }
      const failure = failureOf(error);
      await respond({ status: "failed", scenarioResults: this.state.scenarioResults, ...(this.state.initialization ? { initialization: this.state.initialization.evaluation } : {}), ...(recovery?.recoveryConfirmed === false ? { commissioning: recovery } : {}),
        ...(error instanceof AcquisitionLimitsRequired || error instanceof ChannelRequirementsRequired ? { missing: error.missing } : {}),
        ...(error instanceof AutomaticControllerSelectionError ? { attempts: error.attempts } : {}),
        failure: presentMainWorkflowFailure(explanation(error), failure), ...(this.state.interruptedCheckpoint && !["completed", "recovered", "retired"].includes(this.state.interruptedCheckpoint.stage) ? { commissioningCheckpoint: true } : {}), reason: error instanceof AutomaticControllerSelectionError
          ? "No controller candidate fits the current model and limits. Choose a compatible controller or review Requirements."
          : error instanceof ApplyPrecondition ? error.message
          : `${failedOperationLabel(action)} failed: ${explanation(error)}${nextActionSentence(failure.nextAction)}` });
    }
    return true;
  }

  private async compareOnModel(connection: SearchConnection, model: GeneralModel, task: EngineerRequirements,
    observed: Readonly<Record<string, number>>, values: Readonly<Record<string, number>>, generation: number) {
    const comparison = await compareOnModel({ loop: this.loop, scenarioLoop: () => this.scenarioLoop(), retainIsolatedOperation: this.retainIsolatedOperation,
      adopt: verifier => { this.state.verifier = verifier; this.state.cancellationOwner = verifier; }, record: run => { this.state.tuningRuns.push(run); },
      current: () => generation === this.state.generation }, connection, model, task, observed, values);
    if (comparison) this.state.modelPrediction = { baseline: comparison.predictedBaseline, candidate: comparison.predictedCandidate };
    return comparison;
  }

  /** Specification 35 §9: a switching or state-feedback law in the program is predicted by its own ST on the selected model. */
  private async programLawPrediction(model: GeneralModel, task: EngineerRequirements, values: Readonly<Record<string, number>>,
    generation: number, respond: (payload: Record<string, unknown>) => Promise<void>) {
    this.state.busy = "verify";
    try {
      await respond({ status: "running", reason: "Predicting the current and tuned responses on the model…" });
      const connection = await connectRuntimeSearch(this.loop, { cancelled: () => generation !== this.state.generation });
      const observed = await (await connectRuntimeParameters(this.loop)).client.status();
      if (generation !== this.state.generation) return undefined;
      return await this.compareOnModel(connection, model, task, observed.values, values, generation);
    } catch (error) {
      return { unavailable: `No predicted response: ${explanation(error)}` };
    } finally {
      if (!this.state.cancelling && this.state.verifier?.snapshot()?.status !== "cancellation-unconfirmed") { this.state.busy = undefined; this.state.cancellationOwner = undefined; }
    }
  }

  snapshotStudy(): EngineerStudy | undefined { return this.captureStudy(false); }
  /** The panel's answer to the loop's open: a loop the engine serves gets its Controller list from its setupDiscover answer, so this session posts none. */
  setEngineServed(served: boolean): void { this.engineServed = served; }

  snapshotDraft(): EngineerStudy | undefined { return this.captureStudy(true); }

  snapshotUnresolvedRecoveryStudy(): EngineerStudy | undefined {
    if (this.state.busy !== "apply" || !this.state.commissioning
      || !(this.state.commissioned?.recoveryConfirmed === false || this.state.commissioning.isCampaignRunning())) return undefined;
    return this.captureStudy(true);
  }

  /** Closing during an Apply observation detaches it at once; reopening reattaches it from its journal (A8.5, A10.4). */
  detachObservationForClose(): boolean {
    const owner = this.state.commissioning;
    if (!owner || !("detachForClose" in owner) || !owner.detachForClose()) return false;
    this.state.commissioning = undefined;
    this.releaseForClose();
    return true;
  }

  async finishUnresolvedRecoveryForClose(): Promise<CommissioningResult | undefined> {
    const owner = this.state.commissioning;
    this.state.commissioning = undefined;
    this.releaseForClose();
    return owner ? await owner.settleForClose() : this.state.commissioned;
  }

  hasPendingManualApplication(): boolean { return this.state.pendingManualApplication !== undefined; }

  hasPendingIdentification(): boolean { return this.state.pendingFind !== undefined || this.state.pendingVerification !== undefined && this.state.identification?.snapshot().status === "running"; }
  /**
   * Reattaches to the identification the journal holds (A8.5, A10.4). A campaign whose experiment may still run is resumed,
   * fitted and stored as Identify stores its result; without one, the original recording and mode are restored by cancelling.
   * Returns the stage payload to show when a model was stored. `progress` receives the resumed campaign's progress (A8.6).
   */
  async recoverIdentificationMode(progress?: (text: string) => Promise<void>): Promise<Record<string, unknown> | undefined> {
    if (this.state.identification || this.state.busy) return undefined;
    const recovery = identificationRecovery(this.loop, this.liveIdentification, () => { this.state.busy = undefined; this.state.cancellationOwner = undefined; });
    if (!recovery) return undefined;
    this.identificationRecovery = recovery;
    // The recovery owns the busy state until it finishes, so a Cancel pressed during a resume reaches its cancel().
    this.state.busy = "verify"; this.state.cancellationOwner = recovery;
    return recoverIdentification(recovery, found => {
      this.state.campaigns.push({ id: randomUUID(), status: "completed" as const, runs: found.runs });
      return acceptIdentificationResult(this.host, found);
    }, progress);
  }
  /**
   * Reattaches to the Apply observation the runtime is recording when the panel reopens (A8.5, A8.6, B6.3). It starts nothing new and
   * reports the outcome as the Apply's own. Closing the panel detaches it and aborts nothing (A10.4).
   */
  async recoverApplyObservation(post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    const checkpoint = this.state.interruptedCheckpoint;
    if (!checkpoint || !observationInProgress(checkpoint) || this.state.busy || this.observationDetached) return false;
    this.state.busy = "apply";
    try { await reattachApplyObservation(this.host, checkpoint as ObservedCheckpoint, post, () => this.observationDetached); }
    finally { this.state.busy = undefined; }
    return true;
  }

  /**
   * Closing during an identification releases this session's clients only (A8.5, A10.4): the experiment, its recording and an
   * isolated run continue in the runtime with their journals, and reopening reattaches to them (A8.6). The campaign's own tail
   * ends at once after release; the runtime-owned experiment and the isolated run are not awaited.
   */
  async finishIdentificationForClose(): Promise<void> {
    this.releaseIdentificationRecovery();
    this.releaseForClose();
    await this.state.pendingFind?.catch(() => undefined);
  }

  pendingManualApplicationOwner(): PendingManualApplicationOwner | undefined {
    const pending = this.state.pendingManualApplication;
    return pending && structuredClone({ operation: pending.operation, origin: pending.origin });
  }

  async finishPendingManualApplicationForClose(): Promise<ManualApplicationCloseRecovery | undefined> {
    const pending = this.state.pendingManualApplication;
    this.releaseForClose();
    if (!pending) return undefined;
    return pending.settling.then(observed => ({ operation: pending.operation, origin: pending.origin, observed }), error => ({
      operation: pending.operation, origin: pending.origin, reason: explanation(error),
    }));
  }

  /** A new panel inspects the currently bound runtime, without reviving its closed review. */
  async reconcileManualApplicationRecovery(recovery: PendingManualApplicationOwner | ManualApplicationCloseRecovery,
    post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    return reconcileManualApplication(this.loop, recovery, post, (owner, reason) => this.requireManualRecovery(owner, reason),
      () => { this.state.manualApplicationRecovery = undefined; });
  }

  private captureStudy(recoveryOnly: boolean): EngineerStudy | undefined {
    const selectedModel = this.model();
    if (!this.state.recordingCatalog && !this.state.channelMapping && !selectedModel && !this.state.task && !this.state.tuningPreferences && !this.state.acquisitionPreferences && !this.state.settingsPins && !this.state.scenarioDefinitions?.length
      && this.state.controllerChoice === "automatic" && !this.state.campaigns.length && !this.state.recordedBaseline && !this.state.recordedCandidate && !this.state.verificationAttempts.length) return undefined;
    if (!recoveryOnly && (this.state.busy || this.state.pendingFind || this.state.pendingVerification) && this.state.cancellationOwner?.snapshot()?.status !== "cancellation-unconfirmed") throw new MainWorkflowError("Wait for the current operation or cancel it before saving the study.", { cause: "runtime", nextAction: "wait" });
    const study = decodeEngineerStudy({ ...(this.state.channelMapping ? { channelMapping: this.state.channelMapping } : {}), ...(this.state.tuningPreferences ? { tuningPreferences: this.state.tuningPreferences } : {}), ...(decodeModelComparison(this.state.modelComparison, selectedModel) ? { modelComparison: this.state.modelComparison } : {}), ...(this.state.acquisitionPreferences ? { acquisitionSettings: this.state.acquisitionPreferences } : {}), scenarioDefinitions: this.state.scenarioDefinitions ?? [], ...(selectedModel ? { selectedModel } : {}),
      ...(this.state.tuningRuns.length ? { tuningRuns: this.state.tuningRuns } : {}),
      ...(this.state.recordingCatalog ? { recordingCatalog: this.state.recordingCatalog } : {}),
      ...(this.state.modelPrediction ? { modelPrediction: this.state.modelPrediction } : {}),
      ...(this.state.modelValidation ? { modelValidation: this.state.modelValidation } : {}),
      ...(this.state.retainedDraft ? { draft: this.state.retainedDraft } : {}),
      ...(this.state.scenarioResults.length ? { scenarioResults: this.state.scenarioResults } : {}), requiredScenarios: this.state.requiredScenarios,
      ...(this.state.settingsPins ? { settingsPins: this.state.settingsPins } : {}),
      applicationMode: this.state.applicationMode, controllerChoice: this.state.controllerChoice,
      ...(this.state.task ? { task: this.state.task, family: this.state.family } : {}),
      ...(this.state.gainChoice ? { gainChoice: this.state.gainChoice } : {}),
      ...(this.state.verificationAttempts.length ? { verificationAttempts: this.state.verificationAttempts } : {}), campaigns: this.state.campaigns,
      ...(this.state.recordedBaseline ? { baseline: this.state.recordedBaseline } : {}),
      ...(this.state.recordedCandidate ? { candidate: this.state.recordedCandidate } : {}),
      ...(this.state.recordedInitialization ? { initialization: this.state.recordedInitialization } : {}),
      ...(this.state.familyEvaluations ? { familyEvaluations: this.state.familyEvaluations } : {}) });
    if (!study) throw new Error("Study evidence is invalid or exceeds the supported storage bounds; no records were discarded.");
    return study;
  }
  recordedCommissioning(): CommissioningResult | undefined { return this.state.commissioned; }
  restoreCommissioning(value: CommissioningResult | undefined): void { this.state.commissioned = value; this.state.commissioning = undefined; }
  assertStudyRestoreAllowed(study?: EngineerStudy): void {
    this.assertIdleForReplacement("Open");
    if (study?.settingsPins && study.settingsPins.loopRevision !== this.loop.revision) throw new Error("Saved settings pins belong to a different loop revision.");
    if (study?.gainChoice?.pin && !this.compatiblePin(study.gainChoice.pin, study.selectedModel, study.selectedModel)) throw new Error("Saved gain pin does not match the current loop, model, units or parameter bounds.");
  }
  assertModelReplacementAllowed(): void { this.assertIdleForReplacement("Supply model"); }
  private assertIdleForReplacement(action: string): void {
    if (this.state.busy || this.state.pendingFind || this.state.pendingVerification || this.state.cancelling || this.state.commissioned?.recoveryConfirmed === false || this.state.manualApplicationRecovery
      || this.state.interruptedCheckpoint && !["completed", "recovered", "retired"].includes(this.state.interruptedCheckpoint.stage)) {
      throw new MainWorkflowError(`Finish the running operation, or cancel it and wait for retained evidence, then retry ${action}.`, { cause: "runtime", nextAction: "wait" });
    }
  }
  restoreStudy(value: EngineerStudy | undefined, restoreOperationOwner = true): void {
    this.assertStudyRestoreAllowed(value);
    const study = value === undefined ? undefined : decodeEngineerStudy(value);
    if (value !== undefined && !study) throw new Error("Saved engineer study is invalid.");
    const channelMapping = study?.channelMapping ? controllerChannelMapping(this.configuredLoop, study.channelMapping) : undefined;
    this.invalidate(); this.state.channelMapping = channelMapping;
    this.state.modelComparison = study?.modelComparison;
    this.state.campaigns.splice(0, this.state.campaigns.length, ...(study?.campaigns ?? []));
    this.state.tuningRuns.splice(0, this.state.tuningRuns.length, ...(study?.tuningRuns ?? []));
    this.state.modelPrediction = study?.modelPrediction;
    this.state.acquisitionPreferences = study?.acquisitionSettings;
    this.state.recordingCatalog = study?.recordingCatalog;
    this.state.modelValidation = study?.modelValidation;
    this.state.tuningPreferences = study?.tuningPreferences;
    this.state.scenarioDefinitions = study?.scenarioDefinitions ?? this.loop.verificationScenarios;
    this.state.retainedDraft = study?.draft;
    this.state.task = study?.task;
    this.state.family = study?.family ?? "P";
    this.state.controllerChoice = study?.controllerChoice ?? study?.family ?? "automatic";
    this.state.applicationMode = study?.applicationMode ?? "auto";
    this.state.gainChoice = study?.gainChoice;
    this.state.settingsPins = study?.settingsPins;
    this.state.scenarioResults.splice(0, this.state.scenarioResults.length, ...(study?.scenarioResults ?? []));
    this.state.requiredScenarios = study?.requiredScenarios ?? [];
    this.state.pinModel = study?.gainChoice?.pin ? structuredClone(study.selectedModel) : undefined;
    this.state.verificationAttempts.splice(0, this.state.verificationAttempts.length, ...(study?.verificationAttempts ?? []));
    this.state.cancellationOwner = undefined;
    const unresolved = [...this.state.verificationAttempts].reverse().find(attempt => attempt.status === "cancellation-unconfirmed");
    if (unresolved && restoreOperationOwner) this.restoreIsolatedOperation(unresolved);
    this.state.recordedBaseline = study?.baseline;
    this.state.recordedCandidate = study?.candidate;
    this.state.recordedInitialization = study?.initialization;
    this.state.familyEvaluations = study?.familyEvaluations ? structuredClone(study.familyEvaluations) : undefined;
  }

  /** Relay-cycle results of the feedback-relay recording the selected model was fitted from. */
  relayCycle(): RelayCycleEvidence | undefined { return selectedModelRelayCycle(this.loop, this.model(), this.state.campaigns); }

  restoredOwnerView(): Record<string, unknown> | undefined {
    // A journalled observation is reattached and reported by recoverApplyObservation (A8.6); it is not a recovery to review.
    if (this.state.interruptedCheckpoint && observationInProgress(this.state.interruptedCheckpoint)) return undefined;
    if (this.state.interruptedCheckpoint && !["completed", "recovered", "retired"].includes(this.state.interruptedCheckpoint.stage)) return {
      status: "recovery-required", commissioningCheckpoint: true,
      reason: "An interrupted commissioning operation is retained. Review rollback, or review a restarted runtime, before starting new work.",
    };
    const unresolved = this.state.cancellationOwner?.snapshot();
    if (unresolved?.status === "cancellation-unconfirmed") return { status: "recovery-required", cancellationUnconfirmed: true,
      reason: "The retained isolated operation needs cancellation confirmation. Reconnect its runtime and press Cancel." };
    if (this.state.interruptedCheckpoint?.stage === "completed") return { commissioningCheckpoint: true,
      reason: "The completed commissioning checkpoint is retained. Review rollback to restore its original settings, or Tune and Verify a new candidate." };
    return undefined;
  }

  restoreCommissioningCheckpoint(checkpoint: CommissioningCheckpoint): void {
    this.state.interruptedCheckpoint = checkpoint.stage === "recovered" || checkpoint.stage === "retired" ? undefined : structuredClone(checkpoint);
  }

  restoreIsolatedOperation(attempt: RuntimeVerificationAttempt): void {
    this.retainAttempt(attempt);
    if (attempt.status !== "cancellation-unconfirmed") return;
    const unresolved = attempt;
    let client: RuntimeVerificationClient | undefined;
    this.state.busy = "verify";
    this.state.cancellationOwner = {
      cancel: async () => {
        if (!client) {
          const connection = await bindRuntimeConnection(this.loop.runtimeId);
          if (connection.source !== "simulation") throw new MainWorkflowError("Reconnect the original simulator to resolve its isolated operation.", { cause: "runtime", nextAction: "connect-runtime" });
          client = new RuntimeVerificationClient({ source: "simulation", request: connection.request }, this.retainIsolatedOperation);
          client.restoreCancellation(unresolved);
        }
        await client.cancel();
      },
      snapshot: () => client?.snapshot() ?? unresolved,
    };
  }

  currentStageEvidence(): ControlStudioStageEvidence | undefined {
    try { controllerAdapter(this.loop); } catch { return { loopBound: true, hasModel: false, hasCandidate: false, validationHasResult: false, validationPassed: false }; }
    const model = this.model();
    let hasModel = false;
    if (model) {
      try { assertEngineerModelCompatibility(this.loop, model); hasModel = true; }
      catch { /* An incompatible selected model is inspectable, but cannot make Tune ready. */ }
    }
    return { loopBound: true, hasModel, hasCandidate: hasModel && Boolean(this.state.design),
      validationHasResult: Boolean(this.state.verified), validationPassed: this.state.campaignPassed && this.state.verified?.evaluation.status === "Pass" };
  }

  private retainAttempt(attempt: RuntimeVerificationAttempt | undefined): void {
    if (!attempt || attempt.request.execution === "connected-runtime") return;
    const index = this.state.verificationAttempts.findIndex(value => value.jobId === attempt.jobId && value.projectRevision === attempt.projectRevision);
    if (index < 0) this.state.verificationAttempts.push(attempt); else this.state.verificationAttempts[index] = attempt;
  }

  private updateFamilyEvaluation(family: SavedEngineerFamilyEvaluation["family"],
    status: SavedEngineerFamilyEvaluation["status"], reason: string): void {
    if (!this.state.familyEvaluations) return;
    this.state.familyEvaluations = this.state.familyEvaluations.map(value => value.family === family ? { family, status, reason } : value);
  }

  private clearIncompatiblePin(): void {
    if (this.state.gainChoice?.pin && !this.compatiblePin(this.state.gainChoice.pin)) {
      this.state.gainChoice = { mode: "automatic" }; this.state.pinModel = undefined;
    }
  }

  private compatiblePin(pin: EngineerGainPin, model = this.model(), pinnedModel = this.state.pinModel): boolean {
    try {
      const parameter = proportionalParameter(proportionalDesignLoop(this.loop));
      return pinnedModel !== undefined && isDeepStrictEqual(pinnedModel, model) && pin.modelRevision === model?.revision && pin.loopRevision === this.loop.revision &&
        pin.parameterSymbol === parameter.symbol && pin.unit === parameter.unit &&
        pin.value >= parameter.minimum && pin.value <= parameter.maximum &&
        (parameter.iec_type !== "REAL" || pin.value === Math.fround(pin.value));
    } catch { return false; }
  }

  /** Reviewed acquisition limits bound automatic test settings; tuning a supplied model needs no identification experiment. */
  private requirementLoop(): ControlLoopDefinition {
    const loop = controllerRequirementLoop(this.configuredLoop, this.state.channelMapping, undefined);
    return this.state.acquisitionPreferences ? configuredAcquisition(loop, this.state.acquisitionPreferences) : loop;
  }
  /** The Controller list of a parked loop (V7.17 debt: a loop the engine serves never reads it; see setEngineServed). */
  private availableFamilies(): readonly string[] { try { return controllerAdapter(this.loop).families; } catch { return []; } }
  /** The configuration automatic tuning would choose for the selected model and requirements, shown when Tune opens. A parked loop only (V7.17 debt). */
  private recommendedFamily(): string | null {
    const model = this.model();
    if (!model || !this.state.task || this.loop.controller.project) return null;
    const key = JSON.stringify([model.revision, this.loop.revision, this.state.task, this.state.applicationMode]);
    if (this.state.recommendation?.key !== key) {
      let family: string | null = null;
      try {
        assertEngineerModelCompatibility(this.loop, model);
        family = tuneControllerAdapter(this.loop, model, this.state.task, { family: "automatic", applicationMode: this.state.applicationMode }).family;
      } catch { family = null; }
      this.state.recommendation = { key, family };
    }
    return this.state.recommendation.family;
  }
  /** The running controller's settings when its runtime is reachable; Tune then predicts it beside the tuned one. */
  private async runningControllerGains(): Promise<ControllerGains | undefined> {
    if (!this.loop.parameterBinding) return undefined;
    try { return currentControllerGains(this.loop, (await (await connectRuntimeParameters(this.loop)).client.status()).values); }
    catch { return undefined; }
  }
  private scenarioLoop(): ControlLoopDefinition { return { ...this.loop, verificationScenarios: this.state.scenarioDefinitions }; }
  private inputSignature(): string { return JSON.stringify({ loop: this.scenarioLoop(), model: this.model(), task: this.state.task, values: this.state.values }); }
  private async checkVerifiedProject(): Promise<void> {
    // Each of these is cured by Validate: it checks the model when needed and runs the candidate's ST in every scenario.
    if (!this.state.campaignPassed) throw new MainWorkflowError("Every mandatory scenario must pass before application.", { cause: "settings", nextAction: "validate" });
    if (this.state.verifiedInputs !== this.inputSignature()) throw new MainWorkflowError("Workflow inputs changed after verification. Verify the current candidate again.", { cause: "settings", nextAction: "validate" });
    if (this.state.verified?.evaluation.status !== "Pass" || !this.state.values || this.model()?.revision !== this.state.design?.selectedModel.revision) throw new MainWorkflowError("Verify the selected candidate before application.", { cause: "settings", nextAction: "validate" });
    if (this.model()?.validation !== "validated") throw new MainWorkflowError("Independent model validation or an accepted external model reference is required before application.", { cause: "model-or-data", nextAction: "validate" });
    const connection = await bindingRuntime(() => connectRuntimeSearch(this.loop));
    const readiness = await connection.request("control.simulation.run.status", {});
    if (!record(readiness) || readiness.project_revision !== this.state.verified.projectRevision) throw new MainWorkflowError("The loaded project changed since verification. Verify again.", { cause: "settings", nextAction: "validate" });
  }
  private async invalidateInputs(action: "invalidate" | "cancel", inputs: Record<string, unknown>,
    applicationMode: "auto" | "manual", post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (action === "invalidate" && inputs.channelMapping !== undefined) {
      try { this.state.channelMapping = controllerChannelMapping(this.configuredLoop, inputs.channelMapping); }
      catch (error) { await post({ status: "failed", reason: explanation(error), failure: presentMainWorkflowFailure(explanation(error), failureOf(error)) }); return true; }
    }
    if (action === "invalidate" && (inputs.family === "automatic" || isControllerFamily(inputs.family))) {
      this.state.controllerChoice = inputs.family;
    }
    if (action === "invalidate") {
      if (inputs.tuningPreferences !== undefined) this.state.tuningPreferences = controllerTuningPreferences(inputs.tuningPreferences) ?? this.state.tuningPreferences;
      const settings = acquisitionSettings(inputs.acquisitionSettings);
      if (settings) this.state.acquisitionPreferences = settings;
      if (inputs.scenarioDefinitions !== undefined && validEngineerScenarios(inputs.scenarioDefinitions)
        && JSON.stringify(inputs.scenarioDefinitions).length <= 256000) this.state.scenarioDefinitions = structuredClone(inputs.scenarioDefinitions);
      try { this.state.task = engineerRequirements(inputs); } catch { /* Incomplete edits cannot replace the last usable task. */ }
      if (applicationMode === "auto" || applicationMode === "manual") this.state.applicationMode = applicationMode;
      if (inputs.pinnedValues === null) this.state.settingsPins = undefined;
      else if (record(inputs.pinnedValues)) {
        const pins = decodeControllerSettingsPins({ loopRevision: this.loop.revision,
          family: isControllerFamily(inputs.family) ? inputs.family : this.state.family, values: inputs.pinnedValues });
        if (pins) this.state.settingsPins = pins;
      }
    }
    const pending = this.state.pendingVerification ?? this.state.pendingFind;
    const owner = this.state.cancellationOwner ?? this.state.verifier ?? this.state.identification?.cancellationOwner();
    this.state.cancelling = true; this.state.cancellationOwner = owner;
    this.invalidate();
    await pending?.catch(() => undefined);
    await owner?.cancel().catch(() => undefined);
    const attempt = owner?.snapshot(); this.retainAttempt(attempt);
    this.state.cancelling = false;
    const unconfirmed = attempt?.status === "cancellation-unconfirmed";
    this.state.busy = unconfirmed ? "verify" : undefined;
    this.state.cancellationOwner = unconfirmed ? owner : undefined;
    await post({ inputRevision: this.state.generation, channelMapping: this.state.channelMapping, tuningPreferences: this.state.tuningPreferences, stageEvidence: this.currentStageEvidence(), status: unconfirmed ? "failed" : action === "cancel" ? "cancelled" : "edited",
      verificationAttempts: this.state.verificationAttempts, studyAvailable: true, cancellationUnconfirmed: unconfirmed,
      // Cancelling is the engineer's own action; new work waits until the runtime confirms it has stopped.
      ...(unconfirmed ? { failure: presentMainWorkflowFailure(attempt.reason, { cause: "runtime", nextAction: "wait" }) } : {}),
      canTune: Boolean(this.model()), canIdentify: Boolean(this.loop.parameterBinding && (this.loop.controller.manualOutputSignal || this.loop.signals.setpoint)),
      reason: unconfirmed ? `${attempt.reason} New work is blocked; press Cancel to retry confirmation.`
        : action === "cancel" ? "Operation cancelled. Any started runtime job has reached a terminal state; retained outcomes do not authorize Apply."
        : "Inputs changed. Tune and verify before applying." });
    return true;
  }

  /** An input change: its results are reset and the live work it invalidates is cancelled, as before (A8.4). */
  private invalidate(): void {
    this.resetResults();
    this.cancelLiveWork();
  }

  /**
   * Closing: results are reset and this session lets go of its clients, but no runtime operation is cancelled (A8.5, A10.4).
   * An isolated run keeps going in the runtime and its outcome is still retained through retainIsolatedOperation.
   */
  private releaseForClose(): void {
    this.observationDetached = true;
    this.resetResults();
    this.state.verifier = undefined;
    this.state.identification?.dispose(); this.state.identification = undefined;
  }

  /** Closing lets a reattached identification's timers and followers go; its experiment, recording and journal stay in the runtime. */
  private releaseIdentificationRecovery(): void {
    const recovery = this.identificationRecovery;
    if (!recovery) return;
    this.identificationRecovery = undefined;
    recovery.release?.();
    if (this.state.cancellationOwner === recovery) { this.state.cancellationOwner = undefined; this.state.busy = undefined; }
  }

  private resetResults(): void {
    this.state.runtimeReplacementReview = undefined;
    this.state.modelPrediction = undefined;
    this.state.generation++; this.state.campaignPassed = false; this.state.verifiedInputs = undefined;
    this.state.design = undefined; this.state.verified = undefined; this.state.initialization = undefined; this.state.values = undefined; this.state.review = undefined;
    this.state.cancellationOwner ??= this.state.verifier ?? this.state.identification?.cancellationOwner();
  }

  /**
   * Cancels the isolated run and the identification this session owns. A running identification is cancelled through its owner,
   * which restores the loop before it finishes; a finished one is only released.
   */
  private cancelLiveWork(): void {
    this.state.verifier?.dispose(); this.state.verifier = undefined;
    const identification = this.state.identification;
    if (identification?.snapshot().status === "running") void identification.cancellationOwner().cancel().catch(() => undefined);
    else identification?.dispose();
    this.state.identification = undefined;
  }
  private requireManualRecovery(recovery: PendingManualApplicationOwner, reason: string): string {
    this.state.manualApplicationRecovery = { recovery: structuredClone(recovery), reason };
    return reason;
  }
  dispose(): void { void this.state.commissioning?.cancel().catch(() => undefined); this.releaseIdentificationRecovery(); this.releaseForClose(); }
}

function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function explanation(error: unknown): string { return error instanceof Error ? error.message : String(error); }

/**
 * The failure's typed next action (A11.2). Two errors are classified here because mainWorkflowFailure cannot import
 * their modules without a cycle; every other error is classified by its own type in mainWorkflowFailure.
 */
export function failureOf(error: unknown): MainWorkflowClassification {
  if (error instanceof ChannelRequirementsRequired) return { cause: "settings", nextAction: "open-requirements" };
  if (error instanceof AutomaticControllerSelectionError) {
    // B4.7: no candidate meets the requirements; the detail keeps each structure's reason.
    return { cause: "task-infeasible", nextAction: "open-requirements", summary: "No controller candidate fits the current model and limits." };
  }
  return classifyMainWorkflowFailure(error);
}

/**
 * Resumes a journalled identification and stores its result, or cancels the recovery when no campaign can resume (A8.5).
 * Cancelling restores the original recording and mode; it never starts an experiment.
 */
export async function recoverIdentification(recovery: IdentificationRecovery,
  store: (found: AutomaticIdentificationResult) => Record<string, unknown>,
  progress: (text: string) => Promise<void> = async () => undefined): Promise<Record<string, unknown> | undefined> {
  if (!recovery.resume) { await recovery.cancel(); return undefined; }
  return store(await recovery.resume(progress));
}

/** The failed operation as the learner knows it: by its page or button, never by its internal message kind. */
function failedOperationLabel(action: string): string {
  const labels: Record<string, string> = { find: "Identification", tune: "Tuning", verify: "Validation", apply: "Apply",
    rollback: "Restoring the previous settings", confirm: "Confirmation", inspect: "Inspection", "review-runtime": "Runtime review",
    "retry-recovery": "Recovery", cancel: "Cancelling", invalidate: "Updating the settings", "clear-review": "Clearing the review" };
  return labels[action] ?? action.charAt(0).toUpperCase() + action.slice(1);
}
