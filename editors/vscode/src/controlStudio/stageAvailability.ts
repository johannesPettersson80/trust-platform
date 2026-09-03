// Decides which workflow stages and Automatic Tune are available for a loop's
// experiment execution mode, so a project-scripted lesson cannot be steered off
// its prescribed Identify/Tune/Validate-free path, and computes the workflow-tab
// status vocabulary Specification 35 section 4.2 requires.

export type ControlStudioExecutionMode = "control-studio" | "project-scripted";

export type ControlStudioStageId = "Monitor" | "Test" | "Identify" | "Tune" | "Validate";

export const CONTROL_STUDIO_STAGE_ORDER: readonly ControlStudioStageId[] =
  ["Monitor", "Test", "Identify", "Tune", "Validate"];

/** The six vocabulary words Specification 35 section 4.2 defines, plus the
 * additional state a project-scripted lesson uses for a stage it excludes. */
export type ControlStudioStageStatus =
  | "ready"
  | "needs-data"
  | "in-progress"
  | "complete"
  | "warning"
  | "failed"
  | "not-in-lesson";

const STATUS_LABEL: Record<ControlStudioStageStatus, string> = {
  ready: "Ready",
  "needs-data": "Needs data",
  "in-progress": "In progress",
  complete: "Complete",
  warning: "Warning",
  failed: "Failed",
  "not-in-lesson": "Not in this lesson",
};

export function controlStudioStageStatusLabel(status: ControlStudioStageStatus): string {
  return STATUS_LABEL[status];
}

/** Stages a project-scripted lesson excludes because its runnable project, not
 * Control Studio, owns the controller for the lesson's duration. */
const LESSON_EXCLUDED_STAGES: ReadonlySet<ControlStudioStageId> = new Set(["Identify", "Tune", "Validate"]);

export const PROJECT_SCRIPTED_STAGE_REASON =
  "This lesson's runnable project owns the controller, so this stage is not part of the lesson.";

export interface ControlStudioStageEvidence {
  /** False when no loop is bound yet, so Monitor and Test cannot be used. */
  readonly loopBound?: boolean;
  readonly hasModel?: boolean;
  readonly hasCandidate?: boolean;
  readonly validationHasResult?: boolean;
  readonly validationPassed?: boolean;
}

export interface StageAvailability {
  readonly status: ControlStudioStageStatus;
  readonly label: string;
  readonly disabled: boolean;
  readonly reason?: string;
}

/**
 * Computes, for every workflow tab, the visible status word and whether the
 * stage is reachable. Identify, Tune, and Validate are disabled for a
 * project-scripted loop because the runnable project owns the controller;
 * Monitor and Test remain the lesson's primary stages. Every stage is
 * available for a control-studio loop.
 */
export function controlStudioStageAvailability(
  execution: ControlStudioExecutionMode,
  evidence: ControlStudioStageEvidence = {}
): Record<ControlStudioStageId, StageAvailability> {
  const result = {} as Record<ControlStudioStageId, StageAvailability>;
  for (const stage of CONTROL_STUDIO_STAGE_ORDER) {
    if (execution === "project-scripted" && LESSON_EXCLUDED_STAGES.has(stage)) {
      result[stage] = {
        status: "not-in-lesson",
        label: STATUS_LABEL["not-in-lesson"],
        disabled: true,
        reason: PROJECT_SCRIPTED_STAGE_REASON,
      };
      continue;
    }
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
      return evidence.hasModel ? "complete" : "needs-data";
    case "Tune":
      if (!evidence.hasModel) return "needs-data";
      return evidence.hasCandidate ? "complete" : "ready";
    case "Validate":
      if (!evidence.hasCandidate) return "needs-data";
      if (!evidence.validationHasResult) return "ready";
      return evidence.validationPassed ? "complete" : "failed";
    default: {
      const exhaustive: never = stage;
      return exhaustive;
    }
  }
}

export interface AutomaticTuneAvailability {
  /** False when Automatic Tune must be demoted to a secondary action. */
  readonly primary: boolean;
  readonly reason: string;
}

const AUTOMATIC_TUNE_PRIMARY_REASON = "Build a complete tuning plan.";
const AUTOMATIC_TUNE_LESSON_REASON =
  "This lesson's runnable project owns the controller; Automatic Tune is a secondary action here, not the prescribed path.";

/**
 * Automatic Tune sequences Test, Identify, Tune, and Validate. A
 * project-scripted lesson forbids the student from using those stages
 * directly, so the button offering to run them all automatically must not be
 * the primary action either.
 */
export function controlStudioAutomaticTuneAvailability(
  execution: ControlStudioExecutionMode
): AutomaticTuneAvailability {
  return execution === "project-scripted"
    ? { primary: false, reason: AUTOMATIC_TUNE_LESSON_REASON }
    : { primary: true, reason: AUTOMATIC_TUNE_PRIMARY_REASON };
}
