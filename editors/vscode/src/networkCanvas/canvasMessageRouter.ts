const CANVAS_MESSAGE_TYPES = [
  "ready", "search", "focus", "selectNode", "action", "startLocalSimulator",
  "addSimulatedDevice", "nextStep", "setupProtocol", "adoptDiscovered", "commApply",
  "commSave", "commRemove", "commDisable", "commApplyLive", "commTest",
  "commApplyClientError", "clearApplyResult", "addHost", "addRuntime", "discover",
  "browseSymbols", "createRoute", "addTags", "addEthercatChannels", "addExpose",
  "addOpcuaConnection", "copyText", "setSearch", "setPinnedNode", "focusFault",
  "openQuickAdd", "setStage", "openRuntimePane", "openRuntimeSettings", "openRuntimeLogs",
  "runtimeConnect", "runtimeDisconnect", "setRuntimeAuthToken", "setAsRunTarget",
  "runtimeManagedStart", "runtimeManagedStop", "runtimeManagedLogs",
  "plantSave", "plantRemove", "openControlStudio",
] as const;

export type CanvasMessageType = (typeof CANVAS_MESSAGE_TYPES)[number];
export type CanvasMessage = Record<string, unknown> & { type: CanvasMessageType };

const supportedTypes: ReadonlySet<string> = new Set(CANVAS_MESSAGE_TYPES);

/** Validates the untrusted webview discriminator before host-side routing touches its payload. */
export function decodeCanvasMessage(value: unknown): CanvasMessage | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const message = value as Record<string, unknown>;
  if (typeof message.type !== "string" || !supportedTypes.has(message.type)) return undefined;
  return message as CanvasMessage;
}

export interface OpenControlStudioMessage {
  type: "openControlStudio";
  projectId: string;
  runtimeId: string;
  plantId: string;
  savedConfigurationRevision: string;
  activeConfigurationDigest?: string;
  loopId?: string;
}

/** Keeps live values, paths, and editable plant data outside the navigation request. */
export function decodeOpenControlStudioMessage(value: unknown): OpenControlStudioMessage | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return undefined;
  const message = value as Record<string, unknown>;
  const allowed = new Set([
    "type", "projectId", "runtimeId", "plantId", "savedConfigurationRevision",
    "activeConfigurationDigest", "loopId",
  ]);
  if (Object.keys(message).some((key) => !allowed.has(key))) return undefined;
  if (
    message.type !== "openControlStudio" ||
    !identity(message.projectId, "project:sha256:") ||
    !nonempty(message.runtimeId) ||
    !nonempty(message.plantId) ||
    !identity(message.savedConfigurationRevision, "sha256:") ||
    !optionalIdentity(message.activeConfigurationDigest, "sha256:") ||
    !(message.loopId === undefined || nonempty(message.loopId))
  ) return undefined;
  return message as unknown as OpenControlStudioMessage;
}

function nonempty(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function identity(value: unknown, prefix: string): value is string {
  return nonempty(value) && value.startsWith(prefix) && value.length > prefix.length;
}

function optionalIdentity(value: unknown, prefix: string): boolean {
  return value === undefined || identity(value, prefix);
}
