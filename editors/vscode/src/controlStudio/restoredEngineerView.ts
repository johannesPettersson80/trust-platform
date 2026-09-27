import type { EngineerStudy } from "./engineerStudyPersistence";
import type { CommissioningResult } from "./proportionalCommissioning";

/** Both Open and panel restoration present the same historical facts, never a revived approval. */
export function restoredEngineerView(study: EngineerStudy, commissioning?: CommissioningResult) {
  const unresolved = study.verificationAttempts?.some(attempt => attempt.status === "cancellation-unconfirmed") ?? false;
  return {
    recordingCatalog: study.recordingCatalog,
    channelMapping: study.channelMapping,
    requiredScenarios: study.requiredScenarios,
    identificationPresentation: study.modelComparison?.presentation,
    status: "restored", cancellationUnconfirmed: unresolved, studyAvailable: true, model: study.selectedModel,
    stageEvidence: { loopBound: true, hasModel: Boolean(study.selectedModel), hasCandidate: false,
      validationHasResult: false, validationPassed: false },
    family: study.family, controllerChoice: study.controllerChoice ?? study.family, familyEvaluations: study.familyEvaluations,
    applicationMode: study.applicationMode ?? "auto", commissioning,
    acquisitionSettings: study.acquisitionSettings, scenarioDefinitions: study.scenarioDefinitions ?? [], tuningRuns: study.tuningRuns, draft: study.draft, settingsPins: study.settingsPins ?? null, scenarioResults: study.scenarioResults,
    task: study.task, gainChoice: study.gainChoice,
    tuningPreferences: study.tuningPreferences,
    verificationAttempts: study.verificationAttempts, campaigns: study.campaigns,
    initialization: study.initialization?.evaluation,
    baseline: study.baseline?.evaluation, candidate: study.candidate?.evaluation,
    ...(study.modelPrediction ? { modelPrediction: { baseline: study.modelPrediction.baseline.evaluation, candidate: study.modelPrediction.candidate.evaluation } } : {}),
    reason: commissioning?.recoveryConfirmed === false
      ? "Recovery remains unconfirmed. Review Rollback before starting new work."
      : unresolved ? "An isolated operation still needs cancellation confirmation. Reconnect the original simulator and press Cancel."
      : "Historical study restored. Review inputs, Tune and Verify before applying.",
  };
}
