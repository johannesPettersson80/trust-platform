import type { WorkflowRecordingProvenance } from "./workflowResult";

/** Stable identity of the original saved recording, independent of the importing study. */
export function importedRecordingId(provenance: Pick<WorkflowRecordingProvenance, "projectId" | "resultId" | "resultRevision">): string {
  return `saved-recording:${JSON.stringify([provenance.projectId, provenance.resultId, provenance.resultRevision])}`;
}
