import type { ControlStudioStageEvidence } from "../stageAvailability";
import { setEngineerWorkflowEvidence } from "./workflowActionAvailability";

/** Project current host-owned evidence through the existing stage vocabulary. */
export function renderEngineerStageEvidence(value: unknown): void {
  if (!value || typeof value !== "object" || Array.isArray(value)) return;
  setEngineerWorkflowEvidence(value as ControlStudioStageEvidence);
}
