import type { ExtensionContext } from "vscode";
import type { ControlLoopDefinition } from "./loopDefinition";
import { decodeEngineerStudy, type EngineerStudy } from "./engineerStudyPersistence";
import { decodeWorkflowResult, type ControlStudioWorkflowResult } from "./workflowResult";
import { isDeepStrictEqual } from "util";

type Storage = Pick<ExtensionContext, "workspaceState">;
type Draft = { loopRevision: string; study: EngineerStudy; workflow?: ControlStudioWorkflowResult };
const pending = new Map<string, { value: Draft; writing: Promise<void> }>();

function key(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.draft.${loop.projectId}.${loop.runtimeId}.${loop.id}`;
}

/** Latest editable study only. Runtime owners have separate durable journals. */
export async function storeEngineerDraft(context: Storage, loop: ControlLoopDefinition, study: EngineerStudy | undefined,
  workflow?: ControlStudioWorkflowResult): Promise<void> {
  // Retain an explicit empty draft so reopening cannot fall back to an older result.
  const decoded = decodeEngineerStudy(study ?? { campaigns: [] });
  if (!decoded) throw new Error("The current study cannot be retained; existing saved evidence was preserved.");
  const fullWorkflow = workflow === undefined ? undefined : validateWorkflowDraft(workflow, loop, decoded);
  const storageKey = key(loop), value = { loopRevision: loop.revision, study: decoded, ...(fullWorkflow ? { workflow: fullWorkflow } : {}) };
  const writing = (pending.get(storageKey)?.writing ?? Promise.resolve()).catch(() => undefined)
    .then(async () => { await context.workspaceState.update(storageKey, value); });
  const entry = { value, writing };
  pending.set(storageKey, entry);
  try { await writing; }
  finally { if (pending.get(storageKey) === entry) pending.delete(storageKey); }
}

export function restoreEngineerDraft(context: Storage, loop: ControlLoopDefinition): EngineerStudy | undefined {
  return readDraft(context, loop)?.study;
}

/** Returns the complete current workflow draft; legacy engineer-only drafts return undefined. */
export function restoreWorkflowDraft(context: Storage, loop: ControlLoopDefinition): ControlStudioWorkflowResult | undefined {
  return readDraft(context, loop)?.workflow;
}

function readDraft(context: Storage, loop: ControlLoopDefinition): Draft | undefined {
  const value: unknown = pending.get(key(loop))?.value ?? context.workspaceState.get(key(loop));
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
  return structuredClone(workflow);
}
