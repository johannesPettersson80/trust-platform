import type { PlantControlStudioHandoff } from "./handoff";
import type { ConfiguredLoopRequest } from "./generated/ProjectLoopsResult";

/** The identity of the loop an open request names: what the panel asks the engine for before the engine opens the loop (B1.3). */
export interface RequestedLoop {
  readonly id: string;
  readonly name: string;
  readonly projectId: string;
  readonly runtimeId: string;
  readonly revision: string;
}

/** A plant handoff as the network canvas writes it: its envelope only, which the panel types (the plant summary is the runtime's). */
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

/** Whether the value is an open request for a configured loop: the engine's request shape, not yet checked (B1.3). */
export function isLoopRequest(value: unknown): value is ConfiguredLoopRequest {
  return record(value) && value.schemaVersion === 1 && value.source === "ConfiguredLoop" &&
    record(value.scope) && record(value.loopDefinition);
}

/**
 * The identity an open request names, read from the request the engine wrote. Nothing else of the loop is read: the engine checks the
 * loop file and answers the loop to open (`controlStudio/projectLoops`). Undefined when the request lacks a field of the identity.
 */
export function requestedLoopIdentity(value: unknown): RequestedLoop | undefined {
  if (!isLoopRequest(value)) return undefined;
  const { id, name, projectId, runtimeId, revision } = value.loopDefinition as unknown as Partial<Record<keyof RequestedLoop, unknown>>;
  const complete = [id, name, projectId, runtimeId, revision].every(field => typeof field === "string" && field.length > 0);
  return complete ? { id, name, projectId, runtimeId, revision } as RequestedLoop : undefined;
}

export function text(value: unknown, fallback: string): string {
  return typeof value === "string" && value.trim() ? value.trim() : fallback;
}

function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
