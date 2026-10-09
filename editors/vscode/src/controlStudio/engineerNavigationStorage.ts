import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMessageScope } from "./messages";
import { projectStudyState, updateProjectStudy } from "./studyStore";

export const ENGINEER_STAGES = ["model", "tune", "verify", "apply"] as const;
export type EngineerStage = typeof ENGINEER_STAGES[number];
export function isEngineerStage(value: unknown): value is EngineerStage {
  return ENGINEER_STAGES.includes(value as EngineerStage);
}

/** The workspace-state key the page last viewed used before the study moved into the project (A8.3 migration). */
export function navigationStorageKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.navigation.${loop.projectId}.${loop.runtimeId}.${loop.id}.${loop.revision}`;
}

/** The page last viewed is part of the loop's study (A8.1); it is restored without starting anything. */
export function restoredEngineerStage(loop: ControlLoopDefinition): EngineerStage {
  const value: unknown = projectStudyState(loop).page;
  return isEngineerStage(value) ? value : value && typeof value === "object" && isEngineerStage((value as any).stage) ? (value as any).stage : "model";
}

interface EngineerViewSelection { acquisitionMethod: string; experimentSignalIds: string[] }

function viewSelection(value: unknown): EngineerViewSelection | undefined {
  if (!value || typeof value !== "object") return undefined;
  const view = value as Record<string, unknown>;
  if (typeof view.acquisitionMethod !== "string" || view.acquisitionMethod.length > 64 || !Array.isArray(view.experimentSignalIds)
    || view.experimentSignalIds.length > 256 || !view.experimentSignalIds.every(id => typeof id === "string" && id.length <= 256)) return undefined;
  return { acquisitionMethod: view.acquisitionMethod, experimentSignalIds: [...new Set(view.experimentSignalIds as string[])] };
}

export function restoredEngineerViewSelection(loop: ControlLoopDefinition): EngineerViewSelection | undefined {
  const value = projectStudyState(loop).page as { view?: unknown } | undefined;
  return viewSelection(value?.view);
}

/** View selection is persisted with the study's page and is independent of study results and runtime authority. */
export async function storeEngineerStage(loop: ControlLoopDefinition, message: unknown, scope: LoopMessageScope): Promise<boolean> {
  const value = message as Record<string, unknown> | null;
  if (!value || value.kind !== "control-studio.navigation.select" || value.schemaVersion !== 1 ||
    value.projectId !== scope.projectId || value.editorSessionId !== scope.editorSessionId ||
    value.loopId !== scope.loopId || value.expectedLoopRevision !== scope.loopRevision) return false;
  const stage = (value.payload as { stage?: unknown } | undefined)?.stage;
  if (!isEngineerStage(stage)) return false;
  const payload = value.payload as { view?: unknown };
  const view = viewSelection(payload.view) ?? restoredEngineerViewSelection(loop);
  // The page is only remembered with the study (A8.3): a loop whose study is not open keeps its page for this session.
  try { await updateProjectStudy(loop, "page", view ? { stage, view } : stage); } catch { /* not remembered */ }
  return true;
}
