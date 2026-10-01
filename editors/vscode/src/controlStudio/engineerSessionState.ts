import type { AcquisitionSettings } from "./acquisitionSettings";
import type { IdentificationCampaignRecord } from "./automaticIdentification";
import type { CommissioningCheckpoint } from "./commissioningCheckpoint";
import type { ConnectedIdentification } from "./connectedIdentification";
import type { ControllerDesign } from "./controllerTuning";
import type { ControllerSettingsPins } from "./controllerSettingsPins";
import type { ControllerTuningPreferences } from "./controllerTuningPreferences";
import type { EngineerDraft } from "./engineerDraft";
import type { EngineerGainChoice } from "./engineerGainChoice";
import type { EngineerStudy, SavedEngineerFamilyEvaluation } from "./engineerStudyPersistence";
import type { EngineerScenario, EngineerScenarioResult } from "./engineerVerificationCampaign";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { PendingManualApplicationOwner, PendingManualApplicationRecovery } from "./engineerWorkflowSession";
import type { GeneralModel } from "./generalModel";
import type { IndependentModelValidation } from "./independentModelValidation";
import type { InterruptedCommissioningRecovery } from "./interruptedCommissioningRecovery";
import type { LoopChannels } from "./loopChannels";
import type { ModelComparison } from "./modelComparison";
import type { RecordingSignalCatalog } from "./openedRecordingChannels";
import type { ParameterApplicationClient, ParameterReview, ParameterStatus } from "./parameterApplication";
import type { PidCommissioning } from "./pidCommissioning";
import type { CommissioningResult, ProportionalCommissioning } from "./proportionalCommissioning";
import type { RuntimeVerificationAttempt, RuntimeVerificationClient, RuntimeVerificationResult } from "./verificationClient";

export type EngineerCommissioning = ProportionalCommissioning | PidCommissioning | InterruptedCommissioningRecovery;

/**
 * Everything one Control Studio study holds between messages. The session routes messages; its action owners
 * (`engineerActions/*`) read and change this state.
 */
export interface EngineerSessionState {
  acquisitionPreferences: AcquisitionSettings | undefined;
  recordingCatalog: RecordingSignalCatalog | undefined;
  tuningPreferences: ControllerTuningPreferences | undefined;
  scenarioDefinitions: readonly EngineerScenario[] | undefined;
  runtimeReplacementReview: ParameterStatus | undefined;
  interruptedCheckpoint: CommissioningCheckpoint | undefined;
  readonly tuningRuns: RuntimeVerificationResult[];
  retainedDraft: EngineerDraft | undefined;
  settingsPins: ControllerSettingsPins | undefined;
  requiredScenarios: readonly string[];
  readonly scenarioResults: EngineerScenarioResult[];
  campaignPassed: boolean;
  verifiedInputs: string | undefined;
  generation: number;
  readonly handledMessages: Set<string>;
  design: ControllerDesign | undefined;
  gainChoice: EngineerGainChoice | undefined;
  pinModel: GeneralModel | undefined;
  task: EngineerRequirements | undefined;
  verified: RuntimeVerificationResult | undefined;
  verifier: RuntimeVerificationClient | undefined;
  values: Record<string, number> | undefined;
  review: { client: ParameterApplicationClient; value: ParameterReview; rollback: boolean; commissioning?: EngineerCommissioning } | undefined;
  commissioning: EngineerCommissioning | undefined;
  commissioned: CommissioningResult | undefined;
  applicationMode: "auto" | "manual";
  family: NonNullable<EngineerStudy["family"]>;
  controllerChoice: NonNullable<EngineerStudy["controllerChoice"]>;
  familyEvaluations: readonly SavedEngineerFamilyEvaluation[] | undefined;
  busy: "verify" | "apply" | undefined;
  /** Apply waiting for the process to settle before its review; Abort only ends the wait. */
  applySettling: { cancelled: boolean } | undefined;
  identification: ConnectedIdentification | undefined;
  modelComparison: ModelComparison | undefined;
  modelValidation: IndependentModelValidation | undefined;
  recordedBaseline: RuntimeVerificationResult | undefined;
  recordedCandidate: RuntimeVerificationResult | undefined;
  initialization: RuntimeVerificationResult | undefined;
  recordedInitialization: RuntimeVerificationResult | undefined;
  modelPrediction: EngineerStudy["modelPrediction"];
  pendingFind: Promise<unknown> | undefined;
  pendingVerification: Promise<unknown> | undefined;
  pendingManualApplication: {
    operation: PendingManualApplicationRecovery["operation"];
    origin: PendingManualApplicationRecovery["origin"];
    settling: Promise<ParameterStatus>;
  } | undefined;
  manualApplicationRecovery: { recovery: PendingManualApplicationOwner; reason: string } | undefined;
  cancelling: boolean;
  pendingInputOperation: Promise<boolean> | undefined;
  cancellationOwner: Pick<RuntimeVerificationClient, "cancel" | "snapshot"> | undefined;
  readonly verificationAttempts: RuntimeVerificationAttempt[];
  readonly campaigns: (IdentificationCampaignRecord & { id: string })[];
  channelMapping: LoopChannels | undefined;
  recommendation: { key: string; family: string | null } | undefined;
}

export function createEngineerSessionState(scenarioDefinitions: readonly EngineerScenario[] | undefined): EngineerSessionState {
  return {
    acquisitionPreferences: undefined, recordingCatalog: undefined, tuningPreferences: undefined, scenarioDefinitions,
    runtimeReplacementReview: undefined, interruptedCheckpoint: undefined, tuningRuns: [], retainedDraft: undefined,
    settingsPins: undefined, requiredScenarios: [], scenarioResults: [], campaignPassed: false, verifiedInputs: undefined,
    generation: 0, handledMessages: new Set<string>(), design: undefined, gainChoice: undefined, pinModel: undefined,
    task: undefined, verified: undefined, verifier: undefined, values: undefined, review: undefined, commissioning: undefined,
    commissioned: undefined, applicationMode: "auto", family: "P", controllerChoice: "automatic", familyEvaluations: undefined,
    busy: undefined, applySettling: undefined, identification: undefined, modelComparison: undefined, modelValidation: undefined,
    recordedBaseline: undefined, recordedCandidate: undefined, initialization: undefined, recordedInitialization: undefined,
    modelPrediction: undefined, pendingFind: undefined, pendingVerification: undefined, pendingManualApplication: undefined,
    manualApplicationRecovery: undefined, cancelling: false, pendingInputOperation: undefined, cancellationOwner: undefined,
    verificationAttempts: [], campaigns: [], channelMapping: undefined, recommendation: undefined,
  };
}
