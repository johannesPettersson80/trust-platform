import type { PlantControlStudioHandoff } from "./handoff";
import type { MonitorResult } from "./generated/MonitorResult";
import type { PlantMonitorResult } from "./generated/PlantMonitorResult";

export interface LoopMessageScope {
  projectId: string;
  editorSessionId: string;
  loopId: string;
  loopRevision: string;
}

/**
 * The sample the host posts for a loop: the engine's sample as it answered it, or, when the engine answered a failure, the failure's
 * sentence as an unavailable sample. A failure names no source, so that sample names none: the host does not know the target.
 */
export type LoopMonitorPayload = MonitorResult["sample"] | { availability: "unavailable"; loopId: string; reason: string };

/**
 * The plant summary the host posts: the engine's answer, or, when the engine answered a failure, the failure's sentence as an
 * unavailable summary. A failure names no source, so that summary names none.
 */
export type PlantMonitorPayload = PlantMonitorResult | { availability: "unavailable"; plantId: string; reason: string };

/** The message the host posts to the page with each monitor sample of a loop: the page's envelope shape. */
export interface LoopMonitorEnvelope {
  schemaVersion: 1;
  kind: "loop.monitor.snapshot";
  messageId: string;
  projectId: string;
  editorSessionId: string;
  loopId: string;
  expectedLoopRevision: string;
  payload: LoopMonitorPayload;
}

/** The message the host posts to the page with the plant summary: the page's envelope shape. */
export interface PlantMonitorEnvelope {
  schemaVersion: 1;
  kind: "plant.monitor.snapshot";
  messageId: string;
  projectId: string;
  editorSessionId: string;
  loopId?: string;
  plantId: string;
  expectedPlantRevision: string;
  payload: PlantMonitorPayload;
}

export function loopMonitorEnvelope(
  scope: LoopMessageScope,
  messageId: string,
  payload: LoopMonitorPayload
): LoopMonitorEnvelope {
  return {
    schemaVersion: 1,
    kind: "loop.monitor.snapshot",
    messageId,
    projectId: scope.projectId,
    editorSessionId: scope.editorSessionId,
    loopId: scope.loopId,
    expectedLoopRevision: scope.loopRevision,
    payload,
  };
}

export function plantMonitorEnvelope(
  handoff: PlantControlStudioHandoff,
  editorSessionId: string,
  messageId: string,
  payload: PlantMonitorPayload
): PlantMonitorEnvelope {
  return {
    schemaVersion: 1,
    kind: "plant.monitor.snapshot",
    messageId,
    projectId: handoff.projectId,
    editorSessionId,
    ...(handoff.loopId ? { loopId: handoff.loopId } : {}),
    plantId: handoff.plantId,
    expectedPlantRevision: handoff.savedConfigurationRevision,
    payload,
  };
}

function record(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function nonempty(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

/** The page announces that it is ready: an envelope of the panel's own scope with an empty payload. */
export function decodeLoopReady(value: unknown, expected: LoopMessageScope): boolean {
  const envelope = pageEnvelope(value, expected, "control-studio.ready");
  return envelope !== undefined && Object.keys(envelope.payload).length === 0;
}

/**
 * The envelope every page message carries: its exact keys, its schema, its kind and its scope. A message that fails them is
 * not this panel's and is left to the other routes. This routes a message to its panel; it decides nothing about the loop.
 */
function pageEnvelope(value: unknown, expected: LoopMessageScope, kind: string): { messageId: string; payload: Record<string, unknown> } | undefined {
  if (!record(value) || !record(value.payload) ||
      Object.keys(value).sort().join(",") !== "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" ||
      value.schemaVersion !== 1 || value.kind !== kind || !nonempty(value.messageId) || value.projectId !== expected.projectId ||
      value.editorSessionId !== expected.editorSessionId || value.loopId !== expected.loopId ||
      value.expectedLoopRevision !== expected.loopRevision) return undefined;
  return { messageId: value.messageId as string, payload: value.payload };
}

/** A command the page asks the extension to run for it (A11.2): connect a runtime, or open the loop's setup. */
export type ControlStudioHostCommand = "connect-runtime" | "open-loop-setup";

export function decodeControlStudioHostCommand(value: unknown, expected: LoopMessageScope): ControlStudioHostCommand | undefined {
  const envelope = pageEnvelope(value, expected, "control-studio.host-command");
  if (!envelope || Object.keys(envelope.payload).join(",") !== "command") return undefined;
  const command = envelope.payload.command;
  return command === "connect-runtime" || command === "open-loop-setup" ? command : undefined;
}
