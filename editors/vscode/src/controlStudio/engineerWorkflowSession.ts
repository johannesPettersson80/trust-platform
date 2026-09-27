import { experimentFromSamples } from "./identificationExperiment";
import { controllerChannelMapping, controllerRequirementLoop, assertControllerRequirements, ChannelRequirementsRequired } from "./controllerChannelPreferences";
import type { LoopChannels } from "./loopChannels";
import { reconcileManualApplication } from "./manualApplicationReconciliation";
import { engineerPrediction, selectedModelFrequencyAnalysis } from "./engineerPrediction";
import { decodeModelComparison, type ModelComparison } from "./modelComparison";
import { generalModelFitComparison } from "./generalModelFitPresentation";
import { acquisitionSettings, configuredAcquisition, AcquisitionLimitsRequired, type AcquisitionSettings } from "./acquisitionSettings";
import { decodeRecordingCatalog, type RecordingSignalCatalog } from "./openedRecordingChannels";
import { controllerTuningPreferences, type ControllerTuningPreferences } from "./controllerTuningPreferences";
import { InterruptedCommissioningRecovery } from "./interruptedCommissioningRecovery";
import type { CommissioningCheckpoint, CommissioningCheckpointWriter } from "./commissioningCheckpoint";
import { tuneProjectController } from "./projectController";
import type { ControllerTuningResult } from "./controllerTuning";
import { stateControllerPoles, type StateControllerDesign } from "./stateControllerDesign";
import { isControllerFamily } from "./controllerFamilies";
import { bindRuntimeConnection } from "./runtimeConnection";
import { tuneControllerAdapter, proportionalDesignLoop, type ControllerDesign } from "./controllerTuning";
import { parseStateDesignDraft } from "./stateDesignPreferences";
import type { EngineerDraft } from "./engineerDraft";
import { engineerVerificationCampaign, campaignStatus, validEngineerScenarios, type EngineerScenario, type EngineerScenarioResult } from "./engineerVerificationCampaign";
import { decodeControllerSettingsPins, pinnedControllerGains, type ControllerSettingsPins } from "./controllerSettingsPins";
import { controllerAdapter } from "./controllerAdapters";
import { modelReadyReason } from "./engineerWorkflowPresentation";
import { matchesGainRecovery } from "./gainRecoveryContext";
import { designManualProportionalCandidate } from "./manualProportionalDesign";
import { ProportionalCommissioning, type CommissioningResult } from "./proportionalCommissioning";
import { PidCommissioning } from "./pidCommissioning";
import type { LoopRecording } from "./loopMonitor";
import type { EngineerGainChoice, EngineerGainPin } from "./engineerGainChoice";
import { decodePidLaw } from "./controllerLaw";
import { decodeEngineerStudy, type EngineerStudy, type SavedEngineerFamilyEvaluation } from "./engineerStudyPersistence";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import type { ControlStudioStageEvidence } from "./stageAvailability";
import type { LoopMessageScope } from "./messages";
import { engineerRequirements, engineerVerificationPlan, proportionalParameter, type EngineerRequirements } from "./engineerVerificationPlan";
import { connectRuntimeSearch } from "./runtimeSearchConnection";
import { connectRuntimeParameters } from "./runtimeParameterConnection";
import { RuntimeVerificationClient, type RuntimeVerificationResult, type RuntimeVerificationAttempt } from "./verificationClient";
import { sameParameterHistory, sameParameterValues, type ParameterApplicationClient, type ParameterReview, type ParameterStatus } from "./parameterApplication";
import { randomUUID } from "crypto";
import { isDeepStrictEqual } from "util";
import { ConnectedIdentification, type ConnectedIdentificationPorts } from "./connectedIdentification";
import { identificationRecovery } from "./connectedIdentificationRecovery";
import { checkIndependentModel } from "./modelValidationSession";
import { resolveEngineerRequirements, previewAutomaticRequirements } from "./automaticEngineerRequirements";
import { recordingFingerprint, type IndependentModelValidation } from "./independentModelValidation";
import type { AutomaticIdentificationResult, IdentificationCampaignRecord } from "./automaticIdentification";
import { identifiedModelPresentation, type AcceptedIdentificationModel } from "./identification";
import { dispatchPendingManualApplication } from "./panelPersistence";
import { classifyMainWorkflowFailure, mainWorkflowNextActionLabel, verificationFailure, verificationFailureReason } from "./mainWorkflowFailure";
import { AutomaticControllerSelectionError } from "./automaticControllerRecommendation";
import { configuredPlantRequest, usesConfiguredPlant } from "./configuredPlantVerification";
import { assertEngineerModelCompatibility, assertQualifiedModelResult } from "./engineerModelCompatibility";

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
  private acquisitionPreferences: AcquisitionSettings | undefined;
  private recordingCatalog: RecordingSignalCatalog | undefined;
  private tuningPreferences: ControllerTuningPreferences | undefined;
  private scenarioDefinitions: readonly EngineerScenario[] | undefined;
  private runtimeReplacementReview: ParameterStatus | undefined;
  private interruptedCheckpoint: CommissioningCheckpoint | undefined;
  private readonly tuningRuns: RuntimeVerificationResult[] = [];
  private retainedDraft: EngineerDraft | undefined;
  private settingsPins: ControllerSettingsPins | undefined;
  private requiredScenarios: readonly string[] = [];
  private readonly scenarioResults: EngineerScenarioResult[] = [];
  private campaignPassed = false;
  private verifiedInputs: string | undefined;
  private generation = 0;
  private readonly handledMessages = new Set<string>();
  private design: ControllerDesign | undefined;
  private gainChoice: EngineerGainChoice | undefined;
  private pinModel: GeneralModel | undefined;
  private task: EngineerRequirements | undefined;
  private verified: RuntimeVerificationResult | undefined;
  private verifier: RuntimeVerificationClient | undefined;
  private values: Record<string, number> | undefined;
  private review: { client: ParameterApplicationClient; value: ParameterReview; rollback: boolean; commissioning?: ProportionalCommissioning | PidCommissioning | InterruptedCommissioningRecovery } | undefined;
  private commissioning: ProportionalCommissioning | PidCommissioning | InterruptedCommissioningRecovery | undefined;
  private commissioned: CommissioningResult | undefined;
  private applicationMode: "auto" | "manual" = "auto";
  private family: NonNullable<EngineerStudy["family"]> = "P";
  private controllerChoice: NonNullable<EngineerStudy["controllerChoice"]> = "automatic";
  private familyEvaluations: readonly SavedEngineerFamilyEvaluation[] | undefined;
  private busy: "verify" | "apply" | undefined;
  private identification: ConnectedIdentification | undefined;
  private modelComparison: ModelComparison | undefined;
  private modelValidation: IndependentModelValidation | undefined;
  private recordedBaseline: RuntimeVerificationResult | undefined;
  private recordedCandidate: RuntimeVerificationResult | undefined;
  private initialization: RuntimeVerificationResult | undefined;
  private recordedInitialization: RuntimeVerificationResult | undefined;
  private modelPrediction: EngineerStudy["modelPrediction"];
  private pendingFind: Promise<unknown> | undefined;
  private pendingVerification: Promise<unknown> | undefined;
  private pendingManualApplication: {
    operation: PendingManualApplicationRecovery["operation"];
    origin: PendingManualApplicationRecovery["origin"];
    settling: Promise<ParameterStatus>;
  } | undefined;
  private manualApplicationRecovery: { recovery: PendingManualApplicationOwner; reason: string } | undefined;
  private cancelling = false;
  private pendingInputOperation: Promise<boolean> | undefined;
  private cancellationOwner: Pick<RuntimeVerificationClient, "cancel" | "snapshot"> | undefined;
  private readonly verificationAttempts: RuntimeVerificationAttempt[] = [];
  private readonly campaigns: (IdentificationCampaignRecord & { id: string })[] = [];
  private channelMapping: LoopChannels | undefined;
  private get loop(): ControlLoopDefinition { return controllerRequirementLoop(this.configuredLoop, this.channelMapping, this.task); }
  constructor(private readonly configuredLoop: ControlLoopDefinition, private readonly model: () => GeneralModel | undefined,
    private readonly acceptIdentification: (accepted: AcceptedIdentificationModel) => void,
    private readonly retainRecording?: (recording: LoopRecording) => Promise<void>,
    private readonly retainPendingManualApplication?: (owner: PendingManualApplicationOwner) => Promise<void>,
    private readonly retryManualRecovery?: (post: (payload: Record<string, unknown>) => Promise<void>) => Promise<boolean>,
    private readonly acceptFittedModel?: (model: GeneralModel) => void,
    private readonly retainIsolatedOperation?: (attempt: RuntimeVerificationAttempt) => Promise<void>,
    private readonly retainCommissioningCheckpoint?: CommissioningCheckpointWriter,
    private readonly liveIdentification?: ConnectedIdentificationPorts) { this.scenarioDefinitions = configuredLoop.verificationScenarios; }

  retainIdentificationComparison(accepted: AcceptedIdentificationModel): void {
    const selected = this.model();
    if (!selected) return;
    this.modelComparison = { modelRevision: selected.revision,
      presentation: { ...identifiedModelPresentation(accepted.model, accepted.fit,
        this.loop.signals.manipulatedVariable.unit, this.loop.signals.processVariable.unit),
      experiment: experimentFromSamples(this.loop, accepted.review.validation) } };
  }

  retainOpenedRecordings(runs: readonly RuntimeVerificationResult[]): void {
    this.assertModelReplacementAllowed();
    const id = `opened-recording:${runs.map(result => result.run.id).join(":")}`;
    if (this.campaigns.some(campaign => campaign.id === id)) return;
    const campaigns = [...this.campaigns, { id, status: "completed" as const, runs }];
    if (!decodeEngineerStudy({ campaigns })) throw new Error("The recording archive exceeds the study limits or contains invalid data.");
    this.campaigns.push(campaigns[campaigns.length - 1]);
  }

  retainRecordingCatalog(value: RecordingSignalCatalog): void {
    const catalog = decodeRecordingCatalog(value);
    if (!catalog) throw new Error("The recording signal selection is invalid.");
    this.recordingCatalog = catalog;
  }

  retainModelComparison(value: ModelComparison): void {
    const comparison = decodeModelComparison(value, this.model());
    if (!comparison) throw new Error("The recorded fit does not match the selected model.");
    this.modelComparison = comparison;
  }

  modelChanged(): Record<string, unknown> {
    this.modelValidation = undefined;
    this.invalidate();
    this.clearIncompatiblePin();
    let model: GeneralModel | undefined;
    let modelAnalysis: ReturnType<typeof selectedModelFrequencyAnalysis>;
    try {
      model = this.model();
      if (!model) {
        this.modelComparison = undefined;
        return { status: "edited", stageEvidence: this.currentStageEvidence() };
      }
      this.modelComparison = decodeModelComparison(this.modelComparison, model);
      modelAnalysis = selectedModelFrequencyAnalysis(model);
      assertQualifiedModelResult(this.loop, model);
      this.task = previewAutomaticRequirements(configuredAcquisition(controllerRequirementLoop(this.configuredLoop, this.channelMapping, undefined), this.acquisitionPreferences), model, this.task, this.campaigns);
      const evidence = this.currentStageEvidence(), families = this.availableFamilies();
      return { status: "model-ready", task: this.task, compatibleControllerRequired: evidence?.hasModel !== true, model, ...(this.modelComparison ? { identificationPresentation: this.modelComparison.presentation } : {}), ...(modelAnalysis ? { modelAnalysis } : {}), stageEvidence: this.currentStageEvidence(), gainChoice: this.gainChoice,
        availableFamilies: families, controllerChoice: this.controllerChoice,
        reason: evidence?.hasModel ? modelReadyReason(model, families) : "Choose a compatible controller on Tune." };
    } catch (error) {
      const failure = classifyMainWorkflowFailure("tune", error);
      return { status: "unavailable", failure, ...(model ? { model } : {}), ...(modelAnalysis ? { modelAnalysis } : {}), stageEvidence: this.currentStageEvidence(),
        reason: `${explanation(error)} Next: ${mainWorkflowNextActionLabel(failure.nextAction)}.` };
    }
  }

  async handle(message: unknown, scope: LoopMessageScope, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== scope.projectId || message.editorSessionId !== scope.editorSessionId
      || message.loopId !== scope.loopId || message.expectedLoopRevision !== scope.loopRevision || !record(message.payload)
      || typeof message.messageId !== "string" || typeof message.kind !== "string" || !message.kind.startsWith("control-studio.engineer.")) return false;
    if (this.handledMessages.has(message.messageId)) {
      await post({ status: "busy", reason: "This request has already been handled; no runtime operation was repeated." });
      return true;
    }
    if (this.handledMessages.size >= 4096) {
      await post({ status: "busy", reason: "Session request limit reached. Finish recovery and reopen the panel before starting new work." });
      return true;
    }
    this.handledMessages.add(message.messageId);
    const action = message.kind.slice("control-studio.engineer.".length);
    const requestedApplicationMode = message.payload.applicationMode;
    if (requestedApplicationMode !== undefined && requestedApplicationMode !== "auto" && requestedApplicationMode !== "manual") return false;
    const applicationMode = requestedApplicationMode ?? this.applicationMode;
    if (!["find", "tune", "verify", "apply", "confirm", "rollback", "invalidate", "cancel", "clear-review", "retry-recovery", "inspect", "review-runtime"].includes(action)) return false;
    if (action === "inspect") {
      const vscode = await import("vscode");
      await vscode.commands.executeCommand("workbench.action.quickOpen", `#${this.loop.controller.instance}`);
      return true;
    }
    if (this.interruptedCheckpoint && !["completed", "recovered", "retired"].includes(this.interruptedCheckpoint.stage) && !["rollback", "confirm", "cancel", "review-runtime"].includes(action)) {
      await post({ status: "recovery-required", commissioningCheckpoint: true, reason: "Commissioning was interrupted. Review rollback to reconcile the retained runtime operations before starting new work." });
      return true;
    }
    if (!this.busy && this.commissioned?.recoveryConfirmed === false && !["rollback", "confirm", "review-runtime"].includes(action)) {
      await post({ status: "recovery-required", commissioningCheckpoint: true, commissioning: this.commissioned,
        reason: "Recovery remains unconfirmed. Review Rollback on the current runtime before starting new work." });
      return true;
    }
    if (this.busy === "apply" && action === "cancel" && this.commissioning) {
      const generation = this.generation;
      const result = await this.commissioning.cancel();
      if (result && generation === this.generation) {
        this.commissioned = result;
        if (result.recording) await this.retainRecording?.(result.recording);
        if (result.recoveryConfirmed) this.busy = undefined;
        await post({ status: result.status === "passed" ? "commissioned" : "failed", commissioning: result,
          observed: result.parameters, reason: `Auto commissioning ${result.status}: ${result.reason}` });
      }
      return true;
    }
    if (this.busy === "apply" && action !== "review-runtime") { await post({ status: "busy", reason: "The parameter transaction is awaiting observed confirmation." }); return true; }
    if (action === "invalidate" || action === "cancel") {
      const current = this.invalidateInputs(action, message.payload, applicationMode, post);
      const operation = Promise.all([this.pendingInputOperation, current]).then(() => true);
      this.pendingInputOperation = operation;
      try { return await operation; }
      finally { if (this.pendingInputOperation === operation) this.pendingInputOperation = undefined; }
    }
    // Input changes must finish cancellation and their replies before a new
    // candidate is produced; a late edit reply would otherwise hide that result.
    const generation = this.generation;
    await this.pendingInputOperation;
    if (generation !== this.generation) return true;
    if (this.busy && action !== "review-runtime") { await post({ status: "busy", reason: "Verification is running. Cancel it before changing the candidate." }); return true; }
    if (this.manualApplicationRecovery && action !== "apply" && action !== "rollback" && action !== "retry-recovery") {
      await post({ status: "recovery-required", retryManualRecovery: true, observed: "observed" in this.manualApplicationRecovery.recovery
        ? this.manualApplicationRecovery.recovery.observed : undefined, reason: this.manualApplicationRecovery.reason });
      return true;
    }
    if (applicationMode === "auto" || applicationMode === "manual") this.applicationMode = applicationMode;
    if (action === "clear-review") { this.review = undefined; await post({ status: "review-cleared", reason: "Apply will review the selected application mode." }); return true; }
    const respond = async (payload: Record<string, unknown>) => {
      if (generation !== this.generation) return;
      if (payload.status === "model-ready" && this.model()) {
        payload.modelAnalysis = selectedModelFrequencyAnalysis(this.model()!); this.task = previewAutomaticRequirements(configuredAcquisition(controllerRequirementLoop(this.configuredLoop, this.channelMapping, undefined), this.acquisitionPreferences), this.model()!, this.task, this.campaigns);
      }
      if (payload.status === "candidate" && this.design) {
        const gains = pinnedControllerGains(this.loop, this.family, this.design.candidate, this.settingsPins);
        if (this.settingsPins && this.task && !this.loop.controller.stateFeedback && !this.loop.controller.onOff && !this.loop.controller.project && !decodePidLaw(this.loop.controller.law, this.loop.parameterBinding)) {
          const parameter = proportionalParameter(this.loop);
          this.design = designManualProportionalCandidate(this.model(), gains.proportionalGain!, {
            referenceChange: this.task.referenceChange, constantInputDisturbance: this.task.constantInputDisturbance ?? 0,
            maximumSteadyError: this.task.maximumError - this.task.settlingBand, settlingBand: this.task.settlingBand,
            settlingTimeSeconds: this.task.settlingTimeSeconds,
            proportionalGainBounds: { minimum: parameter.minimum, maximum: parameter.maximum },
          });
        }
        if (this.settingsPins) {
          delete payload.stability; delete payload.attempts; this.familyEvaluations = undefined;
          payload.reason = "Pinned settings retained in the candidate. Verify the actual ST before application.";
        }
        this.design = { ...this.design, candidate: { ...this.design.candidate, ...gains } } as typeof this.design;
        this.values = controllerAdapter(this.loop).materialize(this.design!.candidate, this.values ?? {});
        if (this.loop.controller.stateFeedback) this.design = { ...this.design as StateControllerDesign,
          ...(this.settingsPins ? { status: "exploratory" as const, scope: "Manual coefficient pins retained; current sampled poles are recalculated, and no original optimality claim is retained." } : {}),
          closedLoopPoles: stateControllerPoles(this.loop, this.model()!, this.values) };
        payload.design = this.design; payload.values = this.values;
        this.retainedDraft = { modelRevision: this.model()!.revision, loopRevision: this.loop.revision, family: this.family,
          values: structuredClone(this.values), design: structuredClone(this.design) as unknown as Record<string, unknown> };
      }
      if (payload.status === "model-ready" && this.model()) {
        payload.modelAnalysis = selectedModelFrequencyAnalysis(this.model()!); this.task = previewAutomaticRequirements(configuredAcquisition(controllerRequirementLoop(this.configuredLoop, this.channelMapping, undefined), this.acquisitionPreferences), this.model()!, this.task, this.campaigns);
      }
      if (payload.status === "candidate" && this.design) payload.plots = engineerPrediction(this.model(), this.loop, this.design.candidate, this.task);
      if (generation === this.generation) await post({ ...payload, ...(this.task ? { task: this.task } : {}), ...(this.modelValidation ? { modelValidation: this.modelValidation } : {}), requiredScenarios: this.requiredScenarios, channelMapping: this.channelMapping, ...(this.commissioned?.recoveryConfirmed === false ? { commissioningCheckpoint: true } : {}), tuningPreferences: this.tuningPreferences, acquisitionSettings: this.acquisitionPreferences, scenarioDefinitions: this.scenarioDefinitions ?? [], stageEvidence: this.currentStageEvidence(), gainChoice: this.gainChoice, settingsPins: this.settingsPins ?? null, verificationAttempts: this.verificationAttempts, campaigns: this.campaigns, controllerChoice: this.controllerChoice, availableFamilies: this.availableFamilies(), studyAvailable: Boolean(this.model() || this.task || this.campaigns.length) }); };
    try {
      if (action === "retry-recovery") {
        if (!this.retryManualRecovery) throw new Error("No persisted Manual recovery is available to retry.");
        await this.retryManualRecovery(async payload => respond({ ...payload, retryManualRecovery: true }));
        return true;
      }
      if (action === "find") {
        this.recordingCatalog = undefined;
        if (message.payload.acquisitionSettings !== undefined) {
          const settings = acquisitionSettings(message.payload.acquisitionSettings);
          if (!settings) throw new Error("Review acquisition method, duration, amplitude and relay direction before finding a model.");
          this.acquisitionPreferences = settings;
        }
        const acquisitionLoop = configuredAcquisition(this.loop, this.acquisitionPreferences);
        this.campaignPassed = false; this.verifiedInputs = undefined;
    this.design = undefined; this.verified = undefined; this.initialization = undefined; this.values = undefined; this.review = undefined;
        this.busy = "verify";
        try {
          await respond({ status: "running", reason: "Preparing the simulator…" });
          const connection = await connectRuntimeSearch(this.loop, { cancelled: () => generation !== this.generation });
          if (connection.source !== "simulation") throw new Error("This automatic campaign requires the local simulator.");
          if (generation !== this.generation) return true;
          if (!this.liveIdentification) throw new Error("The live plant acquisition service is unavailable.");
          this.identification = new ConnectedIdentification(connection.request, this.liveIdentification);
          const campaign = this.identification;
          this.cancellationOwner = campaign.cancellationOwner();
          let found: AutomaticIdentificationResult;
          await respond({ status: "running", liveIdentification: true });
          const finding = campaign.run(acquisitionLoop, reason => respond({ status: "running", reason }));
          this.pendingFind = finding;
          try { found = await finding; }
          finally {
            if (this.pendingFind === finding) this.pendingFind = undefined;
            this.campaigns.push({ id: randomUUID(), ...campaign.snapshot() });
            this.retainAttempt(campaign.cancellationOwner().snapshot());
            campaign.dispose();
          }
          if (generation !== this.generation) return true;
          this.modelValidation = undefined;
          if ("model" in found) {
            if (!this.acceptFittedModel) throw new Error("Identification has no selected-model handoff.");
            this.acceptFittedModel(found.model);
          } else this.acceptIdentification(found.accepted);
          this.clearIncompatiblePin();
          const selected = this.model();
          if (!selected) throw new Error("Find model did not select its accepted model.");
          assertQualifiedModelResult(this.loop, selected);
          if ("accepted" in found) this.retainIdentificationComparison(found.accepted);
          else this.modelComparison = found.fit ? generalModelFitComparison(selected, found.fit, found.runs, this.loop) : undefined;
          await respond({ status: "model-ready", model: selected, compatibleControllerRequired: this.currentStageEvidence()?.hasModel !== true, identification: found,
            ...(this.modelComparison ? { identificationPresentation: this.modelComparison.presentation } : {}),
            reason: modelReadyReason(selected, this.availableFamilies()) });
        } finally { if (!this.cancelling && this.cancellationOwner?.snapshot()?.status !== "cancellation-unconfirmed") { this.busy = undefined; this.cancellationOwner = undefined; } }
        return true;
      }
      if (action === "tune") {
        this.campaignPassed = false; this.verifiedInputs = undefined;
        this.design = undefined; this.values = undefined; this.verified = undefined; this.initialization = undefined; this.review = undefined;
        const model = this.model();
        if (!model) throw new Error("Find or supply a model first.");
        assertEngineerModelCompatibility(this.loop, model);
        if (message.payload.pinnedValues !== undefined) message.payload.settingsPins = message.payload.pinnedValues === null ? null
          : { loopRevision: this.loop.revision, family: isControllerFamily(message.payload.family) ? message.payload.family : this.family, values: message.payload.pinnedValues };
        if (message.payload.settingsPins === null) this.settingsPins = undefined;
        else if (message.payload.settingsPins !== undefined) {
          const pin = decodeControllerSettingsPins(message.payload.settingsPins);
          if (!pin || pin.loopRevision !== this.loop.revision) throw new Error("Pinned settings do not match the current loop revision.");
          this.settingsPins = pin;
        }
        if (this.settingsPins) {
          if (message.payload.family !== undefined && message.payload.family !== "automatic" && message.payload.family !== this.settingsPins.family) {
            throw new Error("Clear pinned settings before changing controller family.");
          }
          message.payload.family = this.settingsPins.family;
        }
        if (message.payload.family === undefined) message.payload.family = this.controllerChoice;
        if (this.gainChoice?.pin && this.compatiblePin(this.gainChoice.pin) && message.payload.gainMode !== "automatic") {
          message.payload.family = "P";
          message.payload.gainMode = "manual";
          message.payload.proportionalGain ??= this.gainChoice.pin.value;
        }
        if ((message.payload.family === "automatic" || isControllerFamily(message.payload.family))) {
          this.controllerChoice = message.payload.family as NonNullable<EngineerStudy["controllerChoice"]>;
        }
        if (message.payload.scenarioDefinitions !== undefined) {
          if (JSON.stringify(message.payload.scenarioDefinitions).length > 256000 || !validEngineerScenarios(message.payload.scenarioDefinitions)) throw new Error("Invalid mandatory verification scenario policy.");
          this.scenarioDefinitions = structuredClone(message.payload.scenarioDefinitions);
        }
        if (message.payload.tuningPreferences !== undefined) {
          const preferences = controllerTuningPreferences(message.payload.tuningPreferences);
          if (!preferences) throw new Error("Choose a supported tuning method and finite controller settings.");
          this.tuningPreferences = preferences;
        }
        if (this.tuningPreferences?.method === "state-design" && this.tuningPreferences.stateDesignDraft) {
          this.tuningPreferences.stateDesign = parseStateDesignDraft(this.tuningPreferences.stateDesignDraft,
            String(message.payload.family ?? this.loop.controller.stateFeedback?.family ?? this.family));
        }
        const task = resolveEngineerRequirements(configuredAcquisition(controllerRequirementLoop(this.configuredLoop, this.channelMapping, undefined), this.acquisitionPreferences), model, message.payload, this.tuningPreferences, this.campaigns);
        assertControllerRequirements(this.loop, task);
        this.task = task;
        let tuned: ControllerTuningResult;
        if (this.tuningPreferences?.method === "manual") {
          tuned = tuneControllerAdapter(this.loop, model, task, { family: String(message.payload.family), applicationMode,
            manualValues: this.tuningPreferences.values ?? {}, pinnedValues: this.settingsPins?.values });
        } else if (this.loop.controller.project) {
          if (message.payload.family !== "automatic" && !controllerAdapter(this.loop).families.includes(message.payload.family as typeof this.family)) throw new Error("Choose the controller family declared by the current ST adapter.");
          const selectedProjectFamily = message.payload.family === "automatic" ? undefined : message.payload.family as typeof this.family;
          this.busy = "verify";
          const tuning = (async () => {
            const connection = await connectRuntimeSearch(this.loop, { cancelled: () => generation !== this.generation });
            if (connection.source !== "simulation") throw new Error("Native controller tuning requires isolated simulation.");
            const parameters = await connectRuntimeParameters(this.loop);
            const observed = await parameters.client.status();
            const readiness = await connection.request("control.simulation.run.status", {});
            if (!record(readiness) || readiness.ready !== true || typeof readiness.project_revision !== "string") {
              throw new Error("The loaded source project is not ready for native controller tuning.");
            }
            if (generation !== this.generation) throw new Error("Controller tuning cancelled.");
            this.verifier = new RuntimeVerificationClient({ source: "simulation", request: connection.request }, this.retainIsolatedOperation);
            this.cancellationOwner = this.verifier;
            const design = await tuneProjectController(this.scenarioLoop(), model, task, observed.values, this.settingsPins?.values ?? {}, readiness.project_revision, this.verifier,
              () => generation !== this.generation, result => this.tuningRuns.push(result), attempt => this.retainAttempt(attempt),
              reason => respond({ status: "running", reason }), selectedProjectFamily);
            return { family: design.candidate.structure, design, values: { ...design.candidate.parameters },
              reason: "Native tuning retained the best qualifying declared-grid candidate. Verify the reviewed final campaign before application." };
          })();
          this.pendingVerification = tuning;
          try { tuned = await tuning; }
          finally {
            this.retainAttempt(this.verifier?.snapshot());
            if (this.pendingVerification === tuning) this.pendingVerification = undefined;
            if (!this.cancelling && this.verifier?.snapshot()?.status !== "cancellation-unconfirmed") { this.busy = undefined; this.cancellationOwner = undefined; }
          }
          if (generation !== this.generation) return true;
        } else tuned = tuneControllerAdapter(this.loop, model, task, { family: String(message.payload.family), applicationMode,
          gainMode: message.payload.gainMode, proportionalGain: message.payload.proportionalGain,
          pinnedValues: this.settingsPins?.values, stateDesign: this.tuningPreferences?.stateDesign },
          this.gainChoice?.pin && this.compatiblePin(this.gainChoice.pin) ? this.gainChoice : undefined);
        this.family = tuned.family;
        this.design = tuned.design; this.task = task; this.values = tuned.values;
        if (tuned.gainChoice) {
          this.gainChoice = tuned.gainChoice;
          if (tuned.gainChoice.mode === "manual") this.pinModel = structuredClone(model);
        }
        this.familyEvaluations = tuned.attempts?.map(attempt => ({ family: attempt.structure,
          status: attempt.status === "Feasible" ? "model-qualified" : "unavailable", reason: attempt.reason }));
        await respond({ status: "candidate", ...tuned, tuningRuns: this.tuningRuns });
        return true;
      }

      if (action === "verify") {
        let model = this.model(); const design = this.design; const task = this.task;
        if (!model || !design || !task || model.revision !== design.selectedModel.revision) throw new Error("Tune against the selected model before verification.");
        this.verified = undefined; this.initialization = undefined; this.review = undefined; this.scenarioResults.splice(0); this.busy = "verify";
        const verifying = (async () => { try {
          await respond({ status: "running", reason: "Preparing the simulator…" });
          const connection = await connectRuntimeSearch(this.loop, { cancelled: () => generation !== this.generation });
          if (connection.source !== "simulation") throw new Error("Start the local simulator before Verify.");
          if (model.validation === "unvalidated") {
            this.requiredScenarios = ["plant-model"]; this.verifier?.dispose(); this.verifier = undefined;
            this.modelValidation = await checkIndependentModel(configuredAcquisition(this.loop, this.acquisitionPreferences), model, this.campaigns,
              connection.request, this.liveIdentification, () => generation !== this.generation,
              (acquisition, owner) => { this.identification = acquisition; this.cancellationOwner = owner; },
              campaign => this.campaigns.push({ id: randomUUID(), ...campaign }), respond);
            if (generation !== this.generation) return true;
            if (this.modelValidation.status !== "Pass") {
              await respond({ status: "verification-failed", reason: this.modelValidation.rows.find(row => row.status !== "Pass")?.reason ?? "Independent model validation did not pass." });
              return true;
            }
            model = { ...model, validation: "validated", evidence: [...model.evidence, recordingFingerprint(this.modelValidation.record.run)] };
            if (!this.acceptFittedModel) throw new Error("The validated model has no workflow handoff.");
            this.acceptFittedModel(model);
            if (this.pinModel?.revision === model.revision) this.pinModel = model;
            if (this.modelComparison) this.modelComparison = { ...this.modelComparison, presentation: { ...this.modelComparison.presentation, uncertainty: { label: "Independent model validation passed on Validate." } } };
            await respond({ status: "running", model, activity: "controller-check", reason: "Model checked. Validating the tuned controller…" });
          }
          const parameterClient = await connectRuntimeParameters(this.loop);
          const observed = await parameterClient.client.status();
          if (generation !== this.generation) return true;
          const candidateGains = design.candidate;
          const adapter = controllerAdapter(this.loop);
          const values = adapter.materialize(candidateGains, observed.values);
          const baseline = engineerVerificationPlan(this.loop, model, task, observed.values);
          const campaign = engineerVerificationCampaign(this.scenarioLoop(), model, task, values);
          const candidate = campaign[0];
          this.requiredScenarios = [...(this.modelValidation ? ["plant-model"] : []), ...campaign.map(scenario => scenario.id), "controller-transfer"];
          this.scenarioResults.splice(0); this.campaignPassed = false;
          this.verifier = new RuntimeVerificationClient({ source: "simulation", request: connection.request }, this.retainIsolatedOperation);
          this.cancellationOwner = this.verifier;
          const predictedBaseline = await this.verifier.verify(baseline.request, baseline.requirements);
          if (generation !== this.generation) return true;
          this.tuningRuns.push(predictedBaseline);
          const predictedCandidate = await this.verifier.verify(candidate.request, candidate.requirements);
          if (generation !== this.generation) return true;
          this.tuningRuns.push(predictedCandidate);
          if (predictedBaseline.projectRevision !== predictedCandidate.projectRevision) throw new Error("The project changed during model prediction.");
          this.modelPrediction = { baseline: predictedBaseline, candidate: predictedCandidate };
          await respond({ status: "running", modelPrediction: { baseline: predictedBaseline.evaluation, candidate: predictedCandidate.evaluation }, reason: "Comparing current and tuned responses on the identified model." });
          await respond({ status: "running", reason: "Executing baseline and candidate ST from the selected operating point, model initial states and declared controller initialization." });
          const baselineResult = await this.verifier.verify(configuredPlantRequest(baseline.request), baseline.requirements);
          if (generation !== this.generation) return true;
          this.recordedBaseline = baselineResult;
          const result = await this.verifier.verify(configuredPlantRequest(candidate.request), candidate.requirements);
          if (generation !== this.generation) return true;
          if (baselineResult.projectRevision !== result.projectRevision || result.projectRevision !== predictedCandidate.projectRevision) throw new Error("The project changed between prediction, baseline and candidate verification.");
          this.recordedCandidate = result;
          this.scenarioResults.push({ id: candidate.id, result });
          for (const scenario of campaign.slice(1)) {
            await respond({ status: "running", reason: `Executing mandatory scenario: ${scenario.id}.` });
            if (generation !== this.generation) return true;
            const scenarioResult = await this.verifier.verify(usesConfiguredPlant(this.scenarioLoop(), scenario.id) ? configuredPlantRequest(scenario.request) : scenario.request, scenario.requirements);
            this.scenarioResults.push({ id: scenario.id, result: scenarioResult });
            if (generation !== this.generation) return true;
            if (scenarioResult.projectRevision !== result.projectRevision) throw new Error("The program changed during the mandatory verification campaign.");
          }
          this.campaignPassed = campaignStatus(campaign.map(scenario => scenario.id), this.scenarioResults) === "Pass" && (!this.modelValidation || this.modelValidation.status === "Pass");
          this.updateFamilyEvaluation(this.family,
            this.campaignPassed ? "actual-pass" : "actual-fail",
            `Actual ST verification ${result.evaluation.status}.`);
          if (applicationMode === "auto" && this.campaignPassed) {
            const plan = adapter.initialization(model, task, values);
            const initialization = await this.verifier.verify(configuredPlantRequest(plan.request), plan.requirements);
            if (generation !== this.generation) return true;
            if (initialization.projectRevision !== result.projectRevision) throw new Error("The program changed before initialization verification.");
            this.initialization = initialization; this.recordedInitialization = initialization;
          }
          this.verified = result; this.values = values;
          this.verifiedInputs = this.inputSignature();
          const passed = this.campaignPassed && (applicationMode !== "auto" || this.initialization?.evaluation.status === "Pass");
          const failure = passed ? undefined : this.campaignPassed
            ? { cause: "implementation" as const, nextAction: "review-implementation" as const }
            : verificationFailure(this.scenarioResults.flatMap(entry => entry.result.evaluation.requirements));
          await respond({ status: passed ? "verified" : "verification-failed", baseline: baselineResult.evaluation,
            candidate: result.evaluation, modelPrediction: { baseline: predictedBaseline.evaluation, candidate: predictedCandidate.evaluation }, scenarioResults: this.scenarioResults, campaignStatus: campaignStatus(campaign.map(scenario => scenario.id), this.scenarioResults), initialization: this.initialization?.evaluation, coordinates: "Configured plant with declared controller initialization",
            ...(failure ? { failure } : {}),
            reason: passed ? "Verification passed: actual ST satisfies every declared mandatory scenario requirement."
              : this.campaignPassed ? `Transfer verification failed: inspect the first output, mode and parameter evidence. Next: ${mainWorkflowNextActionLabel(failure!.nextAction)}.`
              : `${verificationFailureReason(this.scenarioResults.flatMap(entry => entry.result.evaluation.requirements))} Next: ${mainWorkflowNextActionLabel(failure!.nextAction)}.` });
        } finally {
          const owner = this.verifier ?? this.cancellationOwner;
          const attempt = owner?.snapshot(); this.retainAttempt(attempt);
          if (attempt?.status === "cancellation-unconfirmed") this.cancellationOwner = owner;
          else if (!this.cancelling) { this.busy = undefined; this.cancellationOwner = undefined; }
        } })();
        this.pendingVerification = verifying;
        try { await verifying; } finally { if (this.pendingVerification === verifying) this.pendingVerification = undefined; }
        return true;
      }
      if (action === "review-runtime") {
        if (!this.interruptedCheckpoint) throw new Error("No interrupted commissioning checkpoint is retained.");
        const current = await (await connectRuntimeParameters(this.loop)).client.status();
        if (current.program_revision === this.interruptedCheckpoint.origin.program_revision || current.status === "pending") {
          throw new Error("The original runtime is still active or has a pending operation. Review rollback to recover that operation.");
        }
        this.runtimeReplacementReview = current;
        this.review = undefined;
        await respond({ status: "review", runtimeReplacementReview: true, replacementRuntimeAvailable: true, previous: this.interruptedCheckpoint.origin.values, next: current.values,
          reason: "A new runtime epoch is active. Confirm archiving the previous unresolved operation as historical and starting a fresh study. This writes no controller values and does not claim the previous operation completed." });
        return true;
      }
      if (action === "confirm" && this.runtimeReplacementReview && this.interruptedCheckpoint) {
        const reviewed = this.runtimeReplacementReview;
        this.runtimeReplacementReview = undefined;
        const current = await (await connectRuntimeParameters(this.loop)).client.status();
        if (!sameParameterHistory(current, reviewed) || current.status === "pending" || !this.retainCommissioningCheckpoint) throw new Error("The replacement runtime changed; review it again.");
        await this.retainCommissioningCheckpoint({ ...this.interruptedCheckpoint, stage: "retired", replacementProgramRevision: current.program_revision });
        this.interruptedCheckpoint = undefined; this.commissioning = undefined; this.commissioned = undefined;
        this.invalidate();
        await post({ status: "edited", recoveryResolved: true, reason: "Previous runtime operation retained as unresolved history. Find or supply a model and verify against the current runtime before application." });
        return true;
      }
      if (action === "rollback" && this.interruptedCheckpoint) {
        if (!this.retainCommissioningCheckpoint) throw new Error("The commissioning recovery journal is unavailable.");
        const connection = await connectRuntimeParameters(this.loop);
        const current = await connection.client.status();
        if (current.program_revision !== this.interruptedCheckpoint.origin.program_revision && current.status !== "pending") {
          await respond({ status: "failed", replacementRuntimeAvailable: true, reason: "The runtime restarted. Review its current settings before continuing." }); return true;
        }
        const recovery = await InterruptedCommissioningRecovery.review(this.loop, this.interruptedCheckpoint, connection.client,
          async checkpoint => { await this.retainCommissioningCheckpoint!(checkpoint); this.interruptedCheckpoint = checkpoint.stage === "recovered" ? undefined : checkpoint; });
        this.review = { client: connection.client, value: recovery.parameterReview(), rollback: true, commissioning: recovery };
        await respond({ status: "review", rollback: true, previous: this.review.value.observed.values,
          next: this.review.value.values, reason: recovery.rollbackDescription() });
        return true;
      }
      if (action === "confirm") {
        const retained = this.review; this.review = undefined;
        if (!retained) throw new Error("Review the exact application or rollback first.");
        if (!retained.rollback) await this.checkVerifiedProject();
        if (generation !== this.generation) return true;
        this.busy = "apply";
        try {
          if (retained.commissioning && !retained.rollback) {
            if (!this.retainCommissioningCheckpoint) throw new Error("Persist commissioning recovery before dispatching the reviewed operation.");
            retained.commissioning.setCheckpointWriter(async checkpoint => {
              await this.retainCommissioningCheckpoint!(checkpoint);
              this.interruptedCheckpoint = checkpoint.stage === "recovered" ? undefined : checkpoint;
            });
            this.commissioning = retained.commissioning;
            await respond({ status: "commissioning", reason: "Commissioning the reviewed controller and recording its response. Cancel remains available." });
            const result = await retained.commissioning.run(); this.commissioned = result;
            if (result.recording) await this.retainRecording?.(result.recording);
            await respond({ status: result.status === "passed" ? "commissioned" : "failed", commissioning: result,
              observed: result.parameters, initialization: this.initialization?.evaluation,
              ...(result.status !== "passed" && result.recoveryConfirmed ? { failure: { cause: "model-or-data", nextAction: "find-model" } } : {}),
              reason: result.status === "passed" ? result.reason : `Auto commissioning ${result.status}: ${result.reason}` });
            return true;
          }
          if (retained.rollback && retained.commissioning) {
            this.commissioning = retained.commissioning;
            await respond({ status: "commissioning", reason: "Recovering the reviewed controller in Manual. Cancel can retry an unconfirmed recovery." });
          }
          if (!retained.commissioning) {
            const owner: PendingManualApplicationOwner = {
              operation: retained.rollback ? "rollback" : "apply",
              origin: { program_revision: retained.value.observed.program_revision, binding_revision: retained.value.observed.binding_revision,
                review: { transaction_id: retained.value.transaction_id, active_revision: retained.value.observed.active_revision, previous_revision: retained.value.observed.previous_revision,
                  values: { ...retained.value.observed.values }, requested: { ...retained.value.values } } },
            };
            let ownerRetained = false;
            const applying = dispatchPendingManualApplication(owner, async value => {
              if (!this.retainPendingManualApplication) throw new Error("Could not retain the pending Manual transaction before dispatch.");
              await this.retainPendingManualApplication(value);
              ownerRetained = true;
            }, async (): Promise<ParameterStatus> => {
              const observed = retained.rollback ? await retained.client.rollback(retained.value, false) : await retained.client.apply(retained.value, false);
              await respond({ status: "applied", observed, reason: retained.rollback
                ? "Previous values restored and observed after ST execution in Manual."
                : "Candidate values staged in Manual and observed after ST execution. Automatic-mode transfer remains a separate commissioning operation." });
              return observed;
            });
            const pendingApplication = {
              ...owner,
              settling: applying,
            };
            this.pendingManualApplication = pendingApplication;
            try { await applying; }
            catch (error) {
              if (!ownerRetained) throw error;
              this.busy = undefined;
              const reason = this.requireManualRecovery(owner,
                `The Manual ${owner.operation} could not be confirmed: ${explanation(error)}. Retry recovery to inspect the retained transaction.`);
              await respond({ status: "recovery-required", retryManualRecovery: true, reason });
            } finally { if (this.pendingManualApplication === pendingApplication) this.pendingManualApplication = undefined; }
            return true;
          }
          const observed = retained.rollback && retained.commissioning ? await retained.commissioning.rollback()
            : await retained.client.apply(retained.value, false);
          if (retained.rollback && retained.commissioning) this.commissioned = retained.commissioning.recordedOutcome();
          await respond({ status: "applied", observed, recoveryResolved: retained.rollback, applicationAction: retained.rollback ? "restore" : "apply", reason: retained.rollback ? "Previous values restored and observed after ST execution in Manual." : "Candidate values staged in Manual and observed after ST execution. Automatic-mode transfer remains a separate commissioning operation." });
        } finally { if (this.commissioning?.recoveryConfirmed !== false || this.commissioning instanceof InterruptedCommissioningRecovery) this.busy = undefined; }
        return true;
      }
      if (action === "apply") await this.checkVerifiedProject();
      const connection = await connectRuntimeParameters(this.loop);
      if (connection.source !== "simulation") throw new Error("This connected commissioning adapter requires the local simulator; physical application needs its explicit reviewed path.");
      if (action === "apply") {
        await this.checkVerifiedProject();
        const value = await connection.client.review(this.values!);
        await this.checkVerifiedProject();
        if (generation !== this.generation) return true;
        this.review = { client: connection.client, value, rollback: false };
        this.manualApplicationRecovery = undefined;
        if (applicationMode === "auto") {
          if (this.initialization?.evaluation.status !== "Pass" || this.initialization.projectRevision !== this.verified?.projectRevision) {
            throw new Error("Verify the Auto initialization case before commissioning.");
          }
          this.review.commissioning = await controllerAdapter(this.loop).commission(this.model()!, this.task!, connection.client, value);
        }
      } else {
        const observed = await connection.client.status();
        const matchesCommissioning = sameParameterHistory(observed, this.commissioned?.parameters)
          || (!this.commissioned?.parameters && this.commissioned?.recoveryConfirmed === false
            && matchesGainRecovery(observed, this.commissioned.gainRecovery));
        if (matchesCommissioning && this.commissioned) {
          controllerAdapter(this.loop).assertRecoveryNeeded(this.commissioned, this.model());
        }
        const value = await connection.client.reviewRollback();
        if (generation !== this.generation) return true;
        let commissioning: ProportionalCommissioning | PidCommissioning | InterruptedCommissioningRecovery | undefined;
        if (matchesCommissioning && this.commissioned) {
          const model = this.model();
          if (!model || !this.task) throw new Error("Restore the compatible model and task before reviewing commissioning recovery.");
          commissioning = this.commissioning ?? await controllerAdapter(this.loop).recover(
            model, this.task, connection.client, value, this.commissioned);
        }
        if (generation !== this.generation) return true;
        this.review = { client: connection.client, value, rollback: true, commissioning };
        this.manualApplicationRecovery = undefined;
      }
      await respond({ status: "review", rollback: this.review.rollback, previous: this.review.value.observed.values,
        next: this.review.value.values, reason: this.review.commissioning && !this.review.rollback
          ? this.review.commissioning.description() : this.review.commissioning
          ? this.review.commissioning.rollbackDescription() : "Review the exact change, then confirm application." });
    } catch (error) {
      this.review = undefined;
      if (this.manualApplicationRecovery) {
        const owner = this.manualApplicationRecovery.recovery;
        const reason = this.requireManualRecovery(owner,
          `The retained ${owner.operation} is still unconfirmed: ${explanation(error)}. Retry recovery after reconnecting.`);
        await respond({ status: "recovery-required", retryManualRecovery: true, reason });
        return true;
      }
      if (action === "tune") { this.design = undefined; this.values = undefined; this.verified = undefined; this.campaignPassed = false; }
      const recovery = this.busy === "apply" ? this.commissioning?.recordedOutcome() : undefined;
      if (recovery?.recoveryConfirmed === false) this.commissioned = recovery;
      if (error instanceof AutomaticControllerSelectionError) {
        this.familyEvaluations = error.attempts.map(attempt => ({ family: attempt.structure,
          status: "unavailable", reason: attempt.reason }));
      }
      const failure = classifyMainWorkflowFailure(action, error);
      await respond({ status: "failed", scenarioResults: this.scenarioResults, ...(recovery?.recoveryConfirmed === false ? { commissioning: recovery } : {}),
        ...(error instanceof AcquisitionLimitsRequired || error instanceof ChannelRequirementsRequired ? { missing: error.missing } : {}),
        ...(error instanceof AutomaticControllerSelectionError ? { attempts: error.attempts } : {}),
        failure, ...(this.interruptedCheckpoint && !["completed", "recovered", "retired"].includes(this.interruptedCheckpoint.stage) ? { commissioningCheckpoint: true } : {}), reason: error instanceof AutomaticControllerSelectionError
          ? "No controller candidate fits the current model and limits. Choose a compatible controller or review Requirements."
          : `${action} failed: ${explanation(error)} Next: ${mainWorkflowNextActionLabel(failure.nextAction)}.` });
    }
    return true;
  }

  snapshotStudy(): EngineerStudy | undefined { return this.captureStudy(false); }

  snapshotDraft(): EngineerStudy | undefined { return this.captureStudy(true); }

  snapshotUnresolvedRecoveryStudy(): EngineerStudy | undefined {
    if (this.busy !== "apply" || !this.commissioning
      || !(this.commissioned?.recoveryConfirmed === false || this.commissioning.isCampaignRunning())) return undefined;
    return this.captureStudy(true);
  }

  async finishUnresolvedRecoveryForClose(): Promise<CommissioningResult | undefined> {
    const owner = this.commissioning;
    this.commissioning = undefined;
    this.invalidate();
    return owner ? await owner.settleForClose() : this.commissioned;
  }

  hasPendingManualApplication(): boolean { return this.pendingManualApplication !== undefined; }

  hasPendingIdentification(): boolean { return this.pendingFind !== undefined || this.pendingVerification !== undefined && this.identification?.snapshot().status === "running"; }
  async recoverIdentificationMode(): Promise<void> {
    if (this.identification || this.busy) return;
    const recovery = identificationRecovery(this.loop, this.liveIdentification, () => { this.busy = undefined; this.cancellationOwner = undefined; });
    if (!recovery) return;
    this.busy = "verify"; this.cancellationOwner = recovery;
    await recovery.cancel();
  }
  async finishIdentificationForClose(): Promise<void> {
    const owner = this.identification?.cancellationOwner();
    this.invalidate();
    await owner?.cancel();
    await this.pendingFind?.catch(() => undefined);
    await this.pendingVerification?.catch(() => undefined);
  }

  pendingManualApplicationOwner(): PendingManualApplicationOwner | undefined {
    const pending = this.pendingManualApplication;
    return pending && structuredClone({ operation: pending.operation, origin: pending.origin });
  }

  async finishPendingManualApplicationForClose(): Promise<ManualApplicationCloseRecovery | undefined> {
    const pending = this.pendingManualApplication;
    this.invalidate();
    if (!pending) return undefined;
    return pending.settling.then(observed => ({ operation: pending.operation, origin: pending.origin, observed }), error => ({
      operation: pending.operation, origin: pending.origin, reason: explanation(error),
    }));
  }

  /** A new panel inspects the currently bound runtime, without reviving its closed review. */
  async reconcileManualApplicationRecovery(recovery: PendingManualApplicationOwner | ManualApplicationCloseRecovery,
    post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    return reconcileManualApplication(this.loop, recovery, post, (owner, reason) => this.requireManualRecovery(owner, reason),
      () => { this.manualApplicationRecovery = undefined; });
  }

  private captureStudy(recoveryOnly: boolean): EngineerStudy | undefined {
    const selectedModel = this.model();
    if (!this.recordingCatalog && !this.channelMapping && !selectedModel && !this.task && !this.tuningPreferences && !this.acquisitionPreferences && !this.settingsPins && !this.scenarioDefinitions?.length
      && this.controllerChoice === "automatic" && !this.campaigns.length && !this.recordedBaseline && !this.recordedCandidate && !this.verificationAttempts.length) return undefined;
    if (!recoveryOnly && (this.busy || this.pendingFind || this.pendingVerification) && this.cancellationOwner?.snapshot()?.status !== "cancellation-unconfirmed") throw new Error("Wait for the current operation or cancel it before saving the study.");
    const study = decodeEngineerStudy({ ...(this.channelMapping ? { channelMapping: this.channelMapping } : {}), ...(this.tuningPreferences ? { tuningPreferences: this.tuningPreferences } : {}), ...(decodeModelComparison(this.modelComparison, selectedModel) ? { modelComparison: this.modelComparison } : {}), ...(this.acquisitionPreferences ? { acquisitionSettings: this.acquisitionPreferences } : {}), scenarioDefinitions: this.scenarioDefinitions ?? [], ...(selectedModel ? { selectedModel } : {}),
      ...(this.tuningRuns.length ? { tuningRuns: this.tuningRuns } : {}),
      ...(this.recordingCatalog ? { recordingCatalog: this.recordingCatalog } : {}),
      ...(this.modelPrediction ? { modelPrediction: this.modelPrediction } : {}),
      ...(this.modelValidation ? { modelValidation: this.modelValidation } : {}),
      ...(this.retainedDraft ? { draft: this.retainedDraft } : {}),
      ...(this.scenarioResults.length ? { scenarioResults: this.scenarioResults } : {}), requiredScenarios: this.requiredScenarios,
      ...(this.settingsPins ? { settingsPins: this.settingsPins } : {}),
      applicationMode: this.applicationMode, controllerChoice: this.controllerChoice,
      ...(this.task ? { task: this.task, family: this.family } : {}),
      ...(this.gainChoice ? { gainChoice: this.gainChoice } : {}),
      ...(this.verificationAttempts.length ? { verificationAttempts: this.verificationAttempts } : {}), campaigns: this.campaigns,
      ...(this.recordedBaseline ? { baseline: this.recordedBaseline } : {}),
      ...(this.recordedCandidate ? { candidate: this.recordedCandidate } : {}),
      ...(this.recordedInitialization ? { initialization: this.recordedInitialization } : {}),
      ...(this.familyEvaluations ? { familyEvaluations: this.familyEvaluations } : {}) });
    if (!study) throw new Error("Study evidence is invalid or exceeds the supported storage bounds; no records were discarded.");
    return study;
  }
  recordedCommissioning(): CommissioningResult | undefined { return this.commissioned; }
  restoreCommissioning(value: CommissioningResult | undefined): void { this.commissioned = value; this.commissioning = undefined; }
  assertStudyRestoreAllowed(study?: EngineerStudy): void {
    this.assertIdleForReplacement("Open");
    if (study?.settingsPins && study.settingsPins.loopRevision !== this.loop.revision) throw new Error("Saved settings pins belong to a different loop revision.");
    if (study?.gainChoice?.pin && !this.compatiblePin(study.gainChoice.pin, study.selectedModel, study.selectedModel)) throw new Error("Saved gain pin does not match the current loop, model, units or parameter bounds.");
  }
  assertModelReplacementAllowed(): void { this.assertIdleForReplacement("Supply model"); }
  private assertIdleForReplacement(action: string): void {
    if (this.busy || this.pendingFind || this.pendingVerification || this.cancelling || this.commissioned?.recoveryConfirmed === false || this.manualApplicationRecovery
      || this.interruptedCheckpoint && !["completed", "recovered", "retired"].includes(this.interruptedCheckpoint.stage)) {
      throw new Error(`Finish the running operation, or cancel it and wait for retained evidence, then retry ${action}.`);
    }
  }
  restoreStudy(value: EngineerStudy | undefined, restoreOperationOwner = true): void {
    this.assertStudyRestoreAllowed(value);
    const study = value === undefined ? undefined : decodeEngineerStudy(value);
    if (value !== undefined && !study) throw new Error("Saved engineer study is invalid.");
    const channelMapping = study?.channelMapping ? controllerChannelMapping(this.configuredLoop, study.channelMapping) : undefined;
    this.invalidate(); this.channelMapping = channelMapping;
    this.modelComparison = study?.modelComparison;
    this.campaigns.splice(0, this.campaigns.length, ...(study?.campaigns ?? []));
    this.tuningRuns.splice(0, this.tuningRuns.length, ...(study?.tuningRuns ?? []));
    this.modelPrediction = study?.modelPrediction;
    this.acquisitionPreferences = study?.acquisitionSettings;
    this.recordingCatalog = study?.recordingCatalog;
    this.modelValidation = study?.modelValidation;
    this.tuningPreferences = study?.tuningPreferences;
    this.scenarioDefinitions = study?.scenarioDefinitions ?? this.loop.verificationScenarios;
    this.retainedDraft = study?.draft;
    this.task = study?.task;
    this.family = study?.family ?? "P";
    this.controllerChoice = study?.controllerChoice ?? study?.family ?? "automatic";
    this.applicationMode = study?.applicationMode ?? "auto";
    this.gainChoice = study?.gainChoice;
    this.settingsPins = study?.settingsPins;
    this.scenarioResults.splice(0, this.scenarioResults.length, ...(study?.scenarioResults ?? []));
    this.requiredScenarios = study?.requiredScenarios ?? [];
    this.pinModel = study?.gainChoice?.pin ? structuredClone(study.selectedModel) : undefined;
    this.verificationAttempts.splice(0, this.verificationAttempts.length, ...(study?.verificationAttempts ?? []));
    this.cancellationOwner = undefined;
    const unresolved = [...this.verificationAttempts].reverse().find(attempt => attempt.status === "cancellation-unconfirmed");
    if (unresolved && restoreOperationOwner) this.restoreIsolatedOperation(unresolved);
    this.recordedBaseline = study?.baseline;
    this.recordedCandidate = study?.candidate;
    this.recordedInitialization = study?.initialization;
    this.familyEvaluations = study?.familyEvaluations ? structuredClone(study.familyEvaluations) : undefined;
  }

  restoredOwnerView(): Record<string, unknown> | undefined {
    if (this.interruptedCheckpoint && !["completed", "recovered", "retired"].includes(this.interruptedCheckpoint.stage)) return {
      status: "recovery-required", commissioningCheckpoint: true,
      reason: "An interrupted commissioning operation is retained. Review rollback, or review a restarted runtime, before starting new work.",
    };
    const unresolved = this.cancellationOwner?.snapshot();
    if (unresolved?.status === "cancellation-unconfirmed") return { status: "recovery-required", cancellationUnconfirmed: true,
      reason: "The retained isolated operation needs cancellation confirmation. Reconnect its runtime and press Cancel." };
    if (this.interruptedCheckpoint?.stage === "completed") return { commissioningCheckpoint: true,
      reason: "The completed commissioning checkpoint is retained. Review rollback to restore its original settings, or Tune and Verify a new candidate." };
    return undefined;
  }

  restoreCommissioningCheckpoint(checkpoint: CommissioningCheckpoint): void {
    this.interruptedCheckpoint = checkpoint.stage === "recovered" || checkpoint.stage === "retired" ? undefined : structuredClone(checkpoint);
  }

  restoreIsolatedOperation(attempt: RuntimeVerificationAttempt): void {
    this.retainAttempt(attempt);
    if (attempt.status !== "cancellation-unconfirmed") return;
    const unresolved = attempt;
    let client: RuntimeVerificationClient | undefined;
    this.busy = "verify";
    this.cancellationOwner = {
      cancel: async () => {
        if (!client) {
          const connection = await bindRuntimeConnection(this.loop.runtimeId);
          if (connection.source !== "simulation") throw new Error("Reconnect the original simulator to resolve its isolated operation.");
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
    return { loopBound: true, hasModel, hasCandidate: hasModel && Boolean(this.design),
      validationHasResult: Boolean(this.verified), validationPassed: this.campaignPassed && this.verified?.evaluation.status === "Pass" };
  }

  private retainAttempt(attempt: RuntimeVerificationAttempt | undefined): void {
    if (!attempt || attempt.request.execution === "connected-runtime") return;
    const index = this.verificationAttempts.findIndex(value => value.jobId === attempt.jobId && value.projectRevision === attempt.projectRevision);
    if (index < 0) this.verificationAttempts.push(attempt); else this.verificationAttempts[index] = attempt;
  }

  private updateFamilyEvaluation(family: SavedEngineerFamilyEvaluation["family"],
    status: SavedEngineerFamilyEvaluation["status"], reason: string): void {
    if (!this.familyEvaluations) return;
    this.familyEvaluations = this.familyEvaluations.map(value => value.family === family ? { family, status, reason } : value);
  }

  private clearIncompatiblePin(): void {
    if (this.gainChoice?.pin && !this.compatiblePin(this.gainChoice.pin)) {
      this.gainChoice = { mode: "automatic" }; this.pinModel = undefined;
    }
  }

  private compatiblePin(pin: EngineerGainPin, model = this.model(), pinnedModel = this.pinModel): boolean {
    try {
      const parameter = proportionalParameter(proportionalDesignLoop(this.loop));
      return pinnedModel !== undefined && isDeepStrictEqual(pinnedModel, model) && pin.modelRevision === model?.revision && pin.loopRevision === this.loop.revision &&
        pin.parameterSymbol === parameter.symbol && pin.unit === parameter.unit &&
        pin.value >= parameter.minimum && pin.value <= parameter.maximum &&
        (parameter.iec_type !== "REAL" || pin.value === Math.fround(pin.value));
    } catch { return false; }
  }

  private availableFamilies(): readonly string[] { try { return controllerAdapter(this.loop).families; } catch { return []; } }
  private scenarioLoop(): ControlLoopDefinition { return { ...this.loop, verificationScenarios: this.scenarioDefinitions }; }
  private inputSignature(): string { return JSON.stringify({ loop: this.scenarioLoop(), model: this.model(), task: this.task, values: this.values }); }
  private async checkVerifiedProject(): Promise<void> {
    if (!this.campaignPassed) throw new Error("Every mandatory scenario must pass before application.");
    if (this.verifiedInputs !== this.inputSignature()) throw new Error("Workflow inputs changed after verification. Verify the current candidate again.");
    if (this.verified?.evaluation.status !== "Pass" || !this.values || this.model()?.revision !== this.design?.selectedModel.revision) throw new Error("Verify the selected candidate before application.");
    if (this.model()?.validation !== "validated") throw new Error("Independent model validation or an accepted external model reference is required before application.");
    const connection = await connectRuntimeSearch(this.loop);
    const readiness = await connection.request("control.simulation.run.status", {});
    if (!record(readiness) || readiness.project_revision !== this.verified.projectRevision) throw new Error("The loaded project changed since verification. Verify again.");
  }
  private async invalidateInputs(action: "invalidate" | "cancel", inputs: Record<string, unknown>,
    applicationMode: "auto" | "manual", post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (action === "invalidate" && inputs.channelMapping !== undefined) {
      try { this.channelMapping = controllerChannelMapping(this.configuredLoop, inputs.channelMapping); }
      catch (error) { await post({ status: "failed", reason: explanation(error) }); return true; }
    }
    if (action === "invalidate" && (inputs.family === "automatic" || isControllerFamily(inputs.family))) {
      this.controllerChoice = inputs.family;
    }
    if (action === "invalidate") {
      if (inputs.tuningPreferences !== undefined) this.tuningPreferences = controllerTuningPreferences(inputs.tuningPreferences) ?? this.tuningPreferences;
      const settings = acquisitionSettings(inputs.acquisitionSettings);
      if (settings) this.acquisitionPreferences = settings;
      if (inputs.scenarioDefinitions !== undefined && validEngineerScenarios(inputs.scenarioDefinitions)
        && JSON.stringify(inputs.scenarioDefinitions).length <= 256000) this.scenarioDefinitions = structuredClone(inputs.scenarioDefinitions);
      try { this.task = engineerRequirements(inputs); } catch { /* Incomplete edits cannot replace the last usable task. */ }
      if (applicationMode === "auto" || applicationMode === "manual") this.applicationMode = applicationMode;
      if (inputs.pinnedValues === null) this.settingsPins = undefined;
      else if (record(inputs.pinnedValues)) {
        const pins = decodeControllerSettingsPins({ loopRevision: this.loop.revision,
          family: isControllerFamily(inputs.family) ? inputs.family : this.family, values: inputs.pinnedValues });
        if (pins) this.settingsPins = pins;
      }
    }
    const pending = this.pendingVerification ?? this.pendingFind;
    const owner = this.cancellationOwner ?? this.verifier ?? this.identification?.cancellationOwner();
    this.cancelling = true; this.cancellationOwner = owner;
    this.invalidate();
    await pending?.catch(() => undefined);
    await owner?.cancel().catch(() => undefined);
    const attempt = owner?.snapshot(); this.retainAttempt(attempt);
    this.cancelling = false;
    const unconfirmed = attempt?.status === "cancellation-unconfirmed";
    this.busy = unconfirmed ? "verify" : undefined;
    this.cancellationOwner = unconfirmed ? owner : undefined;
    await post({ inputRevision: this.generation, channelMapping: this.channelMapping, tuningPreferences: this.tuningPreferences, stageEvidence: this.currentStageEvidence(), status: unconfirmed ? "failed" : action === "cancel" ? "cancelled" : "edited", campaigns: this.campaigns,
      verificationAttempts: this.verificationAttempts, studyAvailable: true, cancellationUnconfirmed: unconfirmed,
      canTune: Boolean(this.model()), canIdentify: Boolean(this.loop.parameterBinding && (this.loop.controller.manualOutputSignal || this.loop.signals.setpoint)),
      reason: unconfirmed ? `${attempt.reason} New work is blocked; press Cancel to retry confirmation.`
        : action === "cancel" ? "Operation cancelled. Any started runtime job has reached a terminal state; retained outcomes do not authorize Apply."
        : "Inputs changed. Tune and verify before applying." });
    return true;
  }

  private invalidate(): void {
    this.runtimeReplacementReview = undefined;
    this.modelPrediction = undefined;
    this.generation++; this.campaignPassed = false; this.verifiedInputs = undefined;
    this.design = undefined; this.verified = undefined; this.initialization = undefined; this.values = undefined; this.review = undefined;
    this.cancellationOwner ??= this.verifier ?? this.identification?.cancellationOwner();
    this.verifier?.dispose(); this.verifier = undefined;
    this.identification?.dispose(); this.identification = undefined;
  }
  private requireManualRecovery(recovery: PendingManualApplicationOwner, reason: string): string {
    this.manualApplicationRecovery = { recovery: structuredClone(recovery), reason };
    return reason;
  }
  dispose(): void { void this.commissioning?.cancel().catch(() => undefined); this.invalidate(); }
}

function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function explanation(error: unknown): string { return error instanceof Error ? error.message : String(error); }
