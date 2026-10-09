import type { AcceptedIdentificationModel } from "../identification";
import type { CommissioningCheckpointWriter } from "../commissioningCheckpoint";
import type { ConnectedIdentificationPorts } from "../connectedIdentification";
import type { ControlLoopDefinition } from "../loopDefinition";
import type { ControlStudioStageEvidence } from "../stageAvailability";
import type { EngineerGainPin } from "../engineerGainChoice";
import type { EngineerRequirements } from "../engineerVerificationPlan";
import type { EngineerSessionState } from "../engineerSessionState";
import type { GeneralModel } from "../generalModel";
import type { LoopRecording } from "../loopMonitor";
import type { PendingManualApplicationOwner } from "../engineerWorkflowSession";
import type { RelayCycleEvidence } from "../relayCycleEvidence";
import type { RuntimeVerificationAttempt } from "../verificationClient";
import type { SavedEngineerFamilyEvaluation } from "../engineerStudyPersistence";
import type { SearchConnection } from "../finiteSearchSession";
import type { compareOnModel } from "../modelPredictionRuns";
import type { ControllerGains } from "../controllerAdapters";

export type Respond = (payload: Record<string, unknown>) => Promise<void>;
export type ModelComparisonRun = Awaited<ReturnType<typeof compareOnModel>>;

/**
 * What an action owner may use of its Control Studio session: the study state, the session's accessors and injected
 * owners, and the helpers several actions share. The session builds it once; nothing else holds session internals.
 */
export interface EngineerActionHost {
  readonly state: EngineerSessionState;
  loop(): ControlLoopDefinition;
  model(): GeneralModel | undefined;
  readonly liveIdentification?: ConnectedIdentificationPorts;
  readonly retainIsolatedOperation?: (attempt: RuntimeVerificationAttempt) => Promise<void>;
  readonly acceptFittedModel?: (model: GeneralModel) => void;
  readonly retainRecording?: (recording: LoopRecording) => Promise<void>;
  readonly retainPendingManualApplication?: (owner: PendingManualApplicationOwner) => Promise<void>;
  readonly retainCommissioningCheckpoint?: CommissioningCheckpointWriter;
  acceptIdentification(accepted: AcceptedIdentificationModel): void;
  retainIdentificationComparison(accepted: AcceptedIdentificationModel): void;
  retainAttempt(attempt: RuntimeVerificationAttempt | undefined): void;
  updateFamilyEvaluation(family: SavedEngineerFamilyEvaluation["family"], status: SavedEngineerFamilyEvaluation["status"], reason: string): void;
  clearIncompatiblePin(): void;
  compatiblePin(pin: EngineerGainPin): boolean;
  relayCycle(): RelayCycleEvidence | undefined;
  currentStageEvidence(): ControlStudioStageEvidence | undefined;
  availableFamilies(): readonly string[];
  requirementLoop(): ControlLoopDefinition;
  scenarioLoop(): ControlLoopDefinition;
  inputSignature(): string;
  checkVerifiedProject(): Promise<void>;
  invalidate(): void;
  requireManualRecovery(recovery: PendingManualApplicationOwner, reason: string): string;
  compareOnModel(connection: SearchConnection, model: GeneralModel, task: EngineerRequirements,
    observed: Readonly<Record<string, number>>, values: Readonly<Record<string, number>>, generation: number): Promise<ModelComparisonRun>;
  programLawPrediction(model: GeneralModel, task: EngineerRequirements, values: Readonly<Record<string, number>>,
    generation: number, respond: Respond): Promise<ModelComparisonRun | { unavailable: string } | undefined>;
  /** The running controller's settings when its runtime is reachable; a read only, it never writes. */
  runningControllerGains(): Promise<ControllerGains | undefined>;
}

/** One accepted message: its payload, its reply channels and the generation and application mode it was accepted with. */
export interface EngineerActionContext {
  readonly payload: Record<string, unknown>;
  readonly respond: Respond;
  readonly post: Respond;
  readonly generation: number;
  readonly applicationMode: "auto" | "manual";
}
