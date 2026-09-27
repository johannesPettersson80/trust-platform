import type { ExtensionContext } from "vscode";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMessageScope } from "./messages";

export const ENGINEER_STAGES = ["model", "tune", "verify", "apply"] as const;
export type EngineerStage = typeof ENGINEER_STAGES[number];
export function isEngineerStage(value: unknown): value is EngineerStage {
  return ENGINEER_STAGES.includes(value as EngineerStage);
}
function key(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.navigation.${loop.projectId}.${loop.runtimeId}.${loop.id}.${loop.revision}`;
}
/** View selection is persisted independently of study results and runtime authority. */
export function restoredEngineerStage(context: Pick<ExtensionContext, "workspaceState">, loop: ControlLoopDefinition): EngineerStage {
  const value = context.workspaceState.get(key(loop));
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
export function restoredEngineerViewSelection(context: Pick<ExtensionContext, "workspaceState">, loop: ControlLoopDefinition): EngineerViewSelection | undefined {
  const value = context.workspaceState.get<{ view?: unknown }>(key(loop));
  return viewSelection(value?.view);
}
export async function storeEngineerStage(context: Pick<ExtensionContext, "workspaceState">, loop: ControlLoopDefinition,
  message: unknown, scope: LoopMessageScope): Promise<boolean> {
  const value = message as Record<string, unknown> | null;
  if (!value || value.kind !== "control-studio.navigation.select" || value.schemaVersion !== 1 ||
    value.projectId !== scope.projectId || value.editorSessionId !== scope.editorSessionId ||
    value.loopId !== scope.loopId || value.expectedLoopRevision !== scope.loopRevision) return false;
  const stage = (value.payload as { stage?: unknown } | undefined)?.stage;
  if (!isEngineerStage(stage)) return false;
  const payload = value.payload as { view?: unknown };
  const view = viewSelection(payload.view) ?? restoredEngineerViewSelection(context, loop);
  await context.workspaceState.update(key(loop), view ? { stage, view } : stage);
  return true;
}
