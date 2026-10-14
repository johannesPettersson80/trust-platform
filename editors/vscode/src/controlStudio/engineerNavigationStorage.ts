import type { AnalysisPage } from "./generated/PageRequest";
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

/**
 * The analysis the Tune page shows, as the page names it (B9.9): its tool and its inputs. The tool is kept as the page sent it; the
 * page offers only the tools the engine names, so a tool it does not offer is not shown, and the engine checks the inputs it calculates.
 */
function viewAnalysis(value: unknown): AnalysisPage | undefined {
  if (!value || typeof value !== "object") return undefined;
  const page = value as { tool?: unknown; inputs?: unknown };
  if (typeof page.tool !== "string") return undefined;
  return { tool: page.tool as AnalysisPage["tool"], inputs: page.inputs && typeof page.inputs === "object" ? page.inputs as AnalysisPage["inputs"] : {} };
}

/** The analysis the Tune page showed when the loop was last viewed; a parked loop's page calculates it again when the loop opens (B9.9, A8.6). */
export function restoredEngineerAnalysis(loop: ControlLoopDefinition): AnalysisPage | undefined {
  const value = projectStudyState(loop).page as { analysis?: unknown } | undefined;
  return viewAnalysis(value?.analysis);
}

/**
 * View selection and the analysis the Tune page shows are persisted with the study's page, and are independent of study results and
 * runtime authority. A page that is not Tune keeps no analysis; on Tune, `null` means that no analysis is shown, and a message that says
 * nothing about the analysis (such as the binding route's) keeps the one stored.
 */
export async function storeEngineerStage(loop: ControlLoopDefinition, message: unknown, scope: LoopMessageScope): Promise<boolean> {
  const value = message as Record<string, unknown> | null;
  if (!value || value.kind !== "control-studio.navigation.select" || value.schemaVersion !== 1 ||
    value.projectId !== scope.projectId || value.editorSessionId !== scope.editorSessionId ||
    value.loopId !== scope.loopId || value.expectedLoopRevision !== scope.loopRevision) return false;
  const stage = (value.payload as { stage?: unknown } | undefined)?.stage;
  if (!isEngineerStage(stage)) return false;
  const payload = value.payload as { view?: unknown; analysis?: unknown };
  const view = viewSelection(payload.view) ?? restoredEngineerViewSelection(loop);
  const analysis = stage !== "tune" ? undefined : payload.analysis === undefined ? restoredEngineerAnalysis(loop) : viewAnalysis(payload.analysis);
  const page = view || analysis ? { stage, ...(view ? { view } : {}), ...(analysis ? { analysis } : {}) } : stage;
  // The page is only remembered with the study (A8.3): a loop whose study is not open keeps its page for this session.
  try { await updateProjectStudy(loop, "page", page); } catch { /* not remembered */ }
  return true;
}
