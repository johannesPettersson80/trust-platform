import type { ControlLoopDefinition } from "./loopDefinition";
import { decodeEngineerStudy, type EngineerStudy } from "./engineerStudyPersistence";
import { decodeWorkflowResult, type ControlStudioWorkflowResult } from "./workflowResult";
import { isDeepStrictEqual } from "util";
import { projectStudyState, updateProjectStudy } from "./studyStore";
import { shareClone } from "./recordedEvidence";

type Draft = { loopRevision: string; study: EngineerStudy; workflow?: ControlStudioWorkflowResult };

/** The workspace-state key the draft used before the study moved into the project (A8.3 migration). */
export function draftStorageKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.draft.${loop.projectId}.${loop.runtimeId}.${loop.id}`;
}

/**
 * Latest editable study only. It is kept in the loop's project study, and runtime owners keep their own journals.
 * Resolves with the validated workflow that was stored, which is what a later restore returns.
 */
export async function storeEngineerDraft(loop: ControlLoopDefinition, study: EngineerStudy | undefined,
  workflow?: ControlStudioWorkflowResult): Promise<ControlStudioWorkflowResult | undefined> {
  // Retain an explicit empty draft so reopening cannot fall back to an older result.
  const decoded = decodeEngineerStudy(study ?? { campaigns: [] });
  if (!decoded) throw new Error("The current study cannot be retained; existing saved evidence was preserved.");
  const fullWorkflow = workflow === undefined ? undefined : validateWorkflowDraft(workflow, loop, decoded);
  await updateProjectStudy(loop, "draft", { loopRevision: loop.revision, study: decoded, ...(fullWorkflow ? { workflow: fullWorkflow } : {}) });
  return fullWorkflow;
}

export function restoreEngineerDraft(loop: ControlLoopDefinition): EngineerStudy | undefined {
  return readDraft(loop)?.study;
}

/** Returns the complete current workflow draft; legacy engineer-only drafts return undefined. */
export function restoreWorkflowDraft(loop: ControlLoopDefinition): ControlStudioWorkflowResult | undefined {
  return readDraft(loop)?.workflow;
}

function readDraft(loop: ControlLoopDefinition): Draft | undefined {
  const value: unknown = projectStudyState(loop).draft;
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const draft = value as Draft;
  // A different loop's draft remains stored; its parameter pins cannot enter this loop.
  if (draft.loopRevision !== loop.revision) return undefined;
  const study = decodeEngineerStudy(draft.study);
  if (!study) throw new Error("The retained workflow draft cannot be decoded. Its stored evidence has been preserved.");
  if (draft.workflow !== undefined) {
    const workflow = validateWorkflowDraft(draft.workflow, loop, study);
    return { loopRevision: draft.loopRevision, study, workflow };
  }
  return { loopRevision: draft.loopRevision, study };
}

function validateWorkflowDraft(value: unknown, loop: ControlLoopDefinition, study: EngineerStudy): ControlStudioWorkflowResult {
  const workflow = decodeWorkflowResult(value);
  if (!workflow || workflow.projectId !== loop.projectId || workflow.loop.id !== loop.id || workflow.loop.revision !== loop.revision ||
      !workflow.engineerStudy || !isDeepStrictEqual(workflow.engineerStudy, study)) {
    throw new Error("The complete workflow draft is malformed, outside the active loop, or does not match its engineer study.");
  }
  return shareClone(workflow);
}
