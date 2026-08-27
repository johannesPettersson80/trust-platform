export interface PlantControlStudioHandoff {
  schemaVersion: 1;
  handoffId: string;
  projectId: string;
  runtimeId: string;
  runtimeControlId?: string;
  plantId: string;
  savedConfigurationRevision: string;
  activeConfigurationDigest?: string;
  loopId?: string;
  source: "Simulation";
  requestedContext: {
    mode: "Monitor";
    selection: "plant_summary";
  };
}

export type PlantControlStudioHandoffResult =
  | { kind: "accepted"; handoff: PlantControlStudioHandoff }
  | {
      kind: "rejected";
      reason:
        | "unsupported_version"
        | "wrong_project"
        | "unknown_runtime"
        | "unknown_plant"
        | "malformed_handoff";
    };

const REQUIRED_KEYS = [
  "schemaVersion", "handoffId", "projectId", "runtimeId", "plantId",
  "savedConfigurationRevision", "source", "requestedContext",
] as const;
const OPTIONAL_KEYS = ["runtimeControlId", "activeConfigurationDigest", "loopId"] as const;

/**
 * Validates the navigation envelope without resolving live state. Control Studio
 * owns that lookup, so this boundary accepts identities and revisions only.
 */
export function decodePlantControlStudioHandoff(
  value: unknown,
  scope: { projectId: string; runtimeIds: readonly string[]; plantIds: readonly string[] }
): PlantControlStudioHandoffResult {
  if (!record(value)) return rejected("malformed_handoff");
  if (value.schemaVersion !== 1) return rejected("unsupported_version");
  const allowed = new Set<string>([...REQUIRED_KEYS, ...OPTIONAL_KEYS]);
  if (Object.keys(value).some((key) => !allowed.has(key))) return rejected("malformed_handoff");
  if (value.projectId !== scope.projectId) return rejected("wrong_project");
  if (typeof value.runtimeId !== "string" || !scope.runtimeIds.includes(value.runtimeId)) {
    return rejected("unknown_runtime");
  }
  if (typeof value.plantId !== "string" || !scope.plantIds.includes(value.plantId)) {
    return rejected("unknown_plant");
  }
  if (
    !nonempty(value.handoffId) ||
    !nonempty(value.projectId) ||
    !revision(value.savedConfigurationRevision) ||
    !optionalString(value.runtimeControlId) ||
    !optionalRevision(value.activeConfigurationDigest) ||
    !optionalString(value.loopId) ||
    value.source !== "Simulation" ||
    !record(value.requestedContext) ||
    Object.keys(value.requestedContext).sort().join(",") !== "mode,selection" ||
    value.requestedContext.mode !== "Monitor" ||
    value.requestedContext.selection !== "plant_summary"
  ) return rejected("malformed_handoff");
  return { kind: "accepted", handoff: value as unknown as PlantControlStudioHandoff };
}

function rejected(reason: Extract<PlantControlStudioHandoffResult, { kind: "rejected" }>["reason"]): PlantControlStudioHandoffResult {
  return { kind: "rejected", reason };
}

function record(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function nonempty(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function optionalString(value: unknown): boolean {
  return value === undefined || nonempty(value);
}

function revision(value: unknown): value is string {
  return nonempty(value) && value.startsWith("sha256:") && value.length > "sha256:".length;
}

function optionalRevision(value: unknown): boolean {
  return value === undefined || revision(value);
}
