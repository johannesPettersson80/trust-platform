import type { PlantControlStudioHandoff } from "./handoff";
import { validateControlLoopDefinition, type ControlLoopDefinition, type ControlLoopScope } from "./loopDefinition";

interface ControlStudioLoopOpenRequest {
  schemaVersion: 1;
  source: "ConfiguredLoop" | "PidInstance";
  scope: ControlLoopScope;
  loopDefinition: unknown;
}

export function completePlantHandoff(value: unknown): PlantControlStudioHandoff | undefined {
  if (!record(value) || !record(value.requestedContext)) return undefined;
  return value.schemaVersion === 1 &&
    typeof value.handoffId === "string" &&
    typeof value.projectId === "string" &&
    typeof value.runtimeId === "string" &&
    typeof value.plantId === "string" &&
    typeof value.savedConfigurationRevision === "string" &&
    value.source === "Simulation" &&
    value.requestedContext.mode === "Monitor" &&
    value.requestedContext.selection === "plant_summary"
    ? value as unknown as PlantControlStudioHandoff
    : undefined;
}

export function isLoopRequest(value: unknown): value is ControlStudioLoopOpenRequest {
  return record(value) && value.schemaVersion === 1 &&
    (value.source === "ConfiguredLoop" || value.source === "PidInstance") &&
    record(value.scope) && "loopDefinition" in value;
}

export function completeLoopRequest(value: unknown): ControlLoopDefinition | undefined {
  if (!isLoopRequest(value) || typeof value.scope.projectId !== "string" ||
      typeof value.scope.runtimeId !== "string") return undefined;
  const result = validateControlLoopDefinition(value.loopDefinition, {
    projectId: value.scope.projectId,
    runtimeId: value.scope.runtimeId,
  });
  return result.kind === "accepted" ? result.loop : undefined;
}

export function text(value: unknown, fallback: string): string {
  return typeof value === "string" && value.trim() ? value.trim() : fallback;
}

function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
