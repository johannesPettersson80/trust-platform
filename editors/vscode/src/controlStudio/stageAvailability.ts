// Stage navigation follows available evidence, independently of excitation ownership.

export type ControlStudioExecutionMode = "control-studio" | "project-scripted";

export type ControlStudioStageId = "Monitor" | "Test" | "Identify" | "Tune" | "Validate";

export const CONTROL_STUDIO_STAGE_ORDER: readonly ControlStudioStageId[] =
  ["Monitor", "Test", "Identify", "Tune", "Validate"];

/** The six vocabulary words Specification 35 section 4.2 defines. */
export type ControlStudioStageStatus =
  | "ready"
  | "needs-data"
  | "in-progress"
  | "complete"
  | "warning"
  | "failed";

const STATUS_LABEL: Record<ControlStudioStageStatus, string> = {
  ready: "Ready",
  "needs-data": "Needs data",
  "in-progress": "In progress",
  complete: "Complete",
  warning: "Warning",
  failed: "Failed",
};

export interface ControlStudioStageEvidence {
  /** False when no loop is bound yet, so Monitor and Test cannot be used. */
  readonly loopBound?: boolean;
  readonly hasModel?: boolean;
  readonly hasDeclaredAnalysis?: boolean;
  readonly hasManualParameters?: boolean;
  readonly hasCandidate?: boolean;
  readonly hasRecording?: boolean;
  readonly identificationFailed?: boolean;
  readonly validationHasResult?: boolean;
  readonly validationPassed?: boolean;
}

export interface StageAvailability {
  readonly status: ControlStudioStageStatus;
  readonly label: string;
  readonly disabled: boolean;
  readonly reason?: string;
}

/** Every stage opens its own prerequisites; individual operations enforce capabilities. */
export function controlStudioStageAvailability(
  _execution: ControlStudioExecutionMode,
  evidence: ControlStudioStageEvidence = {}
): Record<ControlStudioStageId, StageAvailability> {
  const result = {} as Record<ControlStudioStageId, StageAvailability>;
  for (const stage of CONTROL_STUDIO_STAGE_ORDER) {
    const status = stageStatus(stage, evidence);
    result[stage] = { status, label: STATUS_LABEL[status], disabled: false };
  }
  return result;
}

function stageStatus(stage: ControlStudioStageId, evidence: ControlStudioStageEvidence): ControlStudioStageStatus {
  switch (stage) {
    case "Monitor":
      return evidence.loopBound === false ? "needs-data" : "ready";
    case "Test":
      return evidence.loopBound === false ? "needs-data" : "ready";
    case "Identify":
      if (evidence.identificationFailed) return "failed";
      return evidence.hasModel ? "complete" : evidence.hasRecording ? "ready" : "needs-data";
    case "Tune":
      if (!evidence.hasModel && !evidence.hasDeclaredAnalysis && !evidence.hasManualParameters) return "needs-data";
      return evidence.hasCandidate ? "complete" : "ready";
    case "Validate":
      if (!evidence.hasCandidate) return evidence.hasRecording ? "ready" : "needs-data";
      if (!evidence.validationHasResult) return "ready";
      return evidence.validationPassed ? "complete" : "failed";
    default: {
      const exhaustive: never = stage;
      return exhaustive;
    }
  }
}
