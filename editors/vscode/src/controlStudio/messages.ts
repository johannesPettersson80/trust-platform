import type { PlantControlStudioHandoff } from "./handoff";
import type { PlantMonitorSnapshot } from "./plantMonitor";
import type { LoopMonitorSample, LoopRecording } from "./loopMonitor";
import type { RecordingUpdate } from "./recordingCoordinator";
import type { ExcludedInterval } from "./identification";
import type { ControlExperimentRequestKind } from "./experiment";
import { decodeIdentificationFormDraft, type IdentificationFormDraft } from "./identificationFormDraft";
import type { MimoSettledTrial } from "./threePageMimoAnalysis";

export interface ControlStudioEnvelope<T> {
  schemaVersion: 1;
  kind: string;
  messageId: string;
  projectId: string;
  editorSessionId: string;
  loopId?: string;
  plantId?: string;
  expectedPlantRevision?: string;
  payload: T;
}

export type PlantMonitorEnvelope = ControlStudioEnvelope<PlantMonitorSnapshot> & {
  kind: "plant.monitor.snapshot";
  plantId: string;
  expectedPlantRevision: string;
};

export interface LoopMonitorEnvelope {
  schemaVersion: 1;
  kind: "loop.monitor.snapshot";
  messageId: string;
  projectId: string;
  editorSessionId: string;
  loopId: string;
  expectedLoopRevision: string;
  payload: LoopMonitorSample;
}

export interface LoopMessageScope {
  projectId: string;
  editorSessionId: string;
  loopId: string;
  loopRevision: string;
}

export function loopRecordingEvidenceEnvelope(
  scope: LoopMessageScope,
  messageId: string,
  evidence: RecordingUpdate,
  recording?: LoopRecording
) {
  return {
    schemaVersion: 1 as const,
    kind: "control-studio.recording.evidence" as const,
    messageId,
    projectId: scope.projectId,
    editorSessionId: scope.editorSessionId,
    loopId: scope.loopId,
    expectedLoopRevision: scope.loopRevision,
    payload: recording ? { ...evidence, recording } : evidence,
  };
}

import { validStepResponseSettings, type StepResponseSettings } from "./stepResponseIdentification";
export type EngineeringCommand =
  | { action: "identify-choose-structure"; messageId: string; choice: import("./sensorLagComparison").StructureChoice }
  | { action: "declared-root"; messageId: string; selectedValue?: number }
  | { action: "identify-edit"; messageId: string; form: IdentificationFormDraft }
  | { action: "identify-qualify"; messageId: string; model: string; excludedIntervals?: readonly ExcludedInterval[]; stepResponseSettings?: StepResponseSettings }
  | { action: "identify-fit" | "identify-accept" | "identify-cancel"; messageId: string };
export function decodeEngineeringCommand(value: unknown, expected: LoopMessageScope): EngineeringCommand | undefined {
  if (!record(value) || !record(value.payload) || JSON.stringify(value).length > 16_000 ||
      Object.keys(value).sort().join(",") !== "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" ||
      value.schemaVersion !== 1 || !nonempty(value.messageId) || value.projectId !== expected.projectId ||
      value.editorSessionId !== expected.editorSessionId || value.loopId !== expected.loopId ||
      value.expectedLoopRevision !== expected.loopRevision) return undefined;
  if (value.kind === "control-studio.declared-root.run" && (Object.keys(value.payload).length === 0 ||
      (Object.keys(value.payload).join(",") === "selectedValue" && typeof value.payload.selectedValue === "number" && Number.isFinite(value.payload.selectedValue)))) {
    return { action: "declared-root", messageId: value.messageId,
      ...(typeof value.payload.selectedValue === "number" ? { selectedValue: value.payload.selectedValue } : {}) };
  }
  if (value.kind === "control-studio.identify.edit" && Object.keys(value.payload).length === 1) {
    const form = decodeIdentificationFormDraft(value.payload.form);
    if (form) return { action: "identify-edit", messageId: value.messageId, form };
  }
  if (value.kind === "control-studio.identify.choose-structure" && Object.keys(value.payload).join(",") === "choice"
    && ["first_order", "additional_lag"].includes(String(value.payload.choice))) {
    return { action: "identify-choose-structure", messageId: value.messageId, choice: value.payload.choice as import("./sensorLagComparison").StructureChoice };
  }
  if (value.kind === "control-studio.identify.run" && Object.keys(value.payload).sort().join(",") === "model,stepResponseSettings"
    && ["fopdt", "sopdt"].includes(String(value.payload.model)) && validStepResponseSettings(value.payload.stepResponseSettings)) {
    return { action: "identify-qualify", messageId: value.messageId, model: String(value.payload.model), stepResponseSettings: value.payload.stepResponseSettings };
  }
  if (value.kind === "control-studio.identify.run" && Object.keys(value.payload).join(",") === "model" && identificationModel(value.payload.model)) {
    return { action: "identify-qualify", messageId: value.messageId, model: value.payload.model };
  }
  if (value.kind === "control-studio.identify.run" &&
      Object.keys(value.payload).sort().join(",") === "excludedIntervals,model" && identificationModel(value.payload.model) &&
      Array.isArray(value.payload.excludedIntervals) && value.payload.excludedIntervals.length <= 64 &&
      value.payload.excludedIntervals.every(excludedInterval)) {
    return { action: "identify-qualify", messageId: value.messageId, model: value.payload.model,
      excludedIntervals: value.payload.excludedIntervals as ExcludedInterval[] };
  }
  if (value.kind === "control-studio.identify.fit" && Object.keys(value.payload).length === 0) {
    return { action: "identify-fit", messageId: value.messageId };
  }
  if (value.kind === "control-studio.identify.cancel" && Object.keys(value.payload).length === 0) {
    return { action: "identify-cancel", messageId: value.messageId };
  }
  if (value.kind === "control-studio.identify.accept" && Object.keys(value.payload).length === 0) {
    return { action: "identify-accept", messageId: value.messageId };
  }
  return undefined;
}

function identificationModel(value: unknown): value is string {
  return typeof value === "string" && ["automatic", "integrating", "first_order", "fopdt", "second_order", "sopdt"].includes(value);
}

function excludedInterval(value: unknown): boolean {
  return record(value) && Object.keys(value).sort().join(",") === "endSeconds,startSeconds" &&
    finite(value.startSeconds) && finite(value.endSeconds) && value.startSeconds >= 0 && value.endSeconds >= value.startSeconds;
}

const LOOP_MONITOR_ENVELOPE_KEYS = new Set([
  "schemaVersion", "kind", "messageId", "projectId", "editorSessionId",
  "loopId", "expectedLoopRevision", "payload",
]);

export function loopMonitorEnvelope(
  scope: LoopMessageScope,
  messageId: string,
  payload: LoopMonitorSample
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

export function decodeLoopMonitorEnvelope(
  value: unknown,
  expected: LoopMessageScope
): LoopMonitorEnvelope | undefined {
  if (!record(value) || JSON.stringify(value).length > 256_000) return undefined;
  if (
    Object.keys(value).some((key) => !LOOP_MONITOR_ENVELOPE_KEYS.has(key)) ||
    value.schemaVersion !== 1 || value.kind !== "loop.monitor.snapshot" ||
    !nonempty(value.messageId) || value.projectId !== expected.projectId ||
    value.editorSessionId !== expected.editorSessionId || value.loopId !== expected.loopId ||
    value.expectedLoopRevision !== expected.loopRevision || !loopMonitorSample(value.payload) ||
    value.payload.loopId !== expected.loopId
  ) return undefined;
  return value as unknown as LoopMonitorEnvelope;
}

export type ControlExperimentCommand =
  | {
      action: "review";
      messageId: string;
      kind: ControlExperimentRequestKind;
      amplitude: number;
      durationSeconds: number;
      physicalTopologyReviewed: boolean;
  }
  | { action: "approve"; messageId: string; planId: string; physicalApproved: boolean }
  | { action: "abort"; messageId: string; experimentId: string };

export function decodeControlExperimentCommand(
  value: unknown,
  expected: LoopMessageScope
): ControlExperimentCommand | undefined {
  if (!record(value) || JSON.stringify(value).length > 16_000 || !record(value.payload)) return undefined;
  if (
    Object.keys(value).sort().join(",") !==
      "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" ||
    value.schemaVersion !== 1 || !nonempty(value.messageId) ||
    value.projectId !== expected.projectId || value.editorSessionId !== expected.editorSessionId ||
    value.loopId !== expected.loopId || value.expectedLoopRevision !== expected.loopRevision
  ) return undefined;
  const payload = value.payload;
  if (value.kind === "control-studio.experiment.review" &&
      Object.keys(payload).sort().join(",") === "amplitude,durationSeconds,kind,physicalTopologyReviewed" &&
      experimentKind(payload.kind) &&
      finite(payload.amplitude) && finite(payload.durationSeconds) &&
      typeof payload.physicalTopologyReviewed === "boolean") {
    return {
      action: "review",
      messageId: value.messageId,
      kind: payload.kind,
      amplitude: payload.amplitude,
      durationSeconds: payload.durationSeconds,
      physicalTopologyReviewed: payload.physicalTopologyReviewed,
    };
  }
  if (value.kind === "control-studio.experiment.approve" &&
      Object.keys(payload).sort().join(",") === "physicalApproved,planId" &&
      nonempty(payload.planId) && typeof payload.physicalApproved === "boolean") {
    return {
      action: "approve",
      messageId: value.messageId,
      planId: payload.planId,
      physicalApproved: payload.physicalApproved,
    };
  }
  if (value.kind === "control-studio.experiment.abort" &&
      Object.keys(payload).join(",") === "experimentId" && nonempty(payload.experimentId)) {
    return { action: "abort", messageId: value.messageId, experimentId: payload.experimentId };
  }
  return undefined;
}

function experimentKind(value: unknown): value is ControlExperimentRequestKind {
  return typeof value === "string" && [
    "automatic", "observation", "setpoint_step", "load_step", "mv_step", "mv_pulse",
    "relay", "prbs", "multisine", "output_step",
  ].includes(value);
}

export function plantMonitorEnvelope(
  handoff: PlantControlStudioHandoff,
  editorSessionId: string,
  messageId: string,
  payload: PlantMonitorSnapshot
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

function loopMonitorSample(value: unknown): value is LoopMonitorSample {
  if (!record(value) || !nonempty(value.loopId) ||
      (value.source !== "Simulation" && value.source !== "Physical")) return false;
  if (value.availability === "unavailable") {
    return nonempty(value.reason) && Object.keys(value).sort().join(",") ===
      "availability,loopId,reason,source";
  }
  if (value.availability !== "available") return false;
  return (value.runtimeSessionId === undefined || nonempty(value.runtimeSessionId) && value.runtimeSessionId.length <= 256) &&
    revision(value.loopRevision) && record(value.controller) &&
    nonempty(value.controller.instance) && revision(value.controller.revision) &&
    ["Auto", "Manual", "Unavailable"].includes(String(value.controller.mode)) &&
    nonnegativeInteger(value.scan) && finite(value.observedAtMs) &&
    (value.setpoint === undefined || monitorSignal(value.setpoint)) && monitorSignal(value.processVariable) &&
    monitorSignal(value.manipulatedVariable) && auxiliaryMonitorSignals(value.additional) &&
    record(value.outputLimits) &&
    finite(value.outputLimits.minimum) && finite(value.outputLimits.maximum) &&
    record(value.rateLimits) && ["minimum", "maximum", "none"].includes(String(value.saturation)) &&
    typeof value.recording === "boolean";
}

function auxiliaryMonitorSignals(value: unknown): boolean {
  if (value === undefined) return true;
  return Array.isArray(value) && value.length <= 256 && value.every((signal) =>
    record(signal) && monitorSignal(signal) && nonempty(signal.id) && nonempty(signal.channel) &&
    nonempty(signal.displayName) &&
    ["SP", "PV", "MV", "actuator", "disturbance", "state", "quality", "schedule"]
      .includes(String(signal.role)) &&
    Object.keys(signal).sort().join(",") === "address,channel,displayName,id,quality,role,symbol,unit,value");
}

function monitorSignal(value: unknown): boolean {
  return record(value) && nonempty(value.symbol) && nonempty(value.address) &&
    finite(value.value) && typeof value.unit === "string" &&
    ["good", "forced", "uncertain", "stale", "bad"].includes(String(value.quality));
}

function record(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function nonempty(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function revision(value: unknown): value is string {
  return nonempty(value) && value.startsWith("sha256:");
}

function finite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function nonnegativeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

export function decodeLoopReady(value: unknown, expected: LoopMessageScope): boolean {
  if (!record(value) || !record(value.payload)) return false;
  return Object.keys(value).sort().join(",") ===
      "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" &&
    value.schemaVersion === 1 && value.kind === "control-studio.ready" &&
    typeof value.messageId === "string" && value.messageId.length > 0 &&
    value.projectId === expected.projectId && value.editorSessionId === expected.editorSessionId &&
    value.loopId === expected.loopId && value.expectedLoopRevision === expected.loopRevision &&
    Object.keys(value.payload).length === 0;
}

export function decodeMimoAnalysisCommand(value: unknown, expected: LoopMessageScope):
  { messageId: string; recordingId: string; trials: readonly MimoSettledTrial[]; maximumUnexcitedInputDrift?: number } | undefined {
  if (!record(value) || !record(value.payload) || Object.keys(value).sort().join(",") !== "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" || value.schemaVersion !== 1 || value.kind !== "control-studio.mimo.analyze" ||
      !nonempty(value.messageId) || value.projectId !== expected.projectId || value.editorSessionId !== expected.editorSessionId ||
      value.loopId !== expected.loopId || value.expectedLoopRevision !== expected.loopRevision ||
      !nonempty(value.payload.recordingId) || !Array.isArray(value.payload.trials) || value.payload.trials.length === 0 ||
      !value.payload.trials.every(mimoTrial)) return undefined;
  if (Object.keys(value.payload).some(key => !["recordingId", "trials", "maximumUnexcitedInputDrift"].includes(key)) ||
      value.payload.maximumUnexcitedInputDrift !== undefined && (!finite(value.payload.maximumUnexcitedInputDrift) || value.payload.maximumUnexcitedInputDrift < 0)) return undefined;
  return { messageId: value.messageId, recordingId: value.payload.recordingId, trials: value.payload.trials,
    ...(value.payload.maximumUnexcitedInputDrift === undefined ? {} : { maximumUnexcitedInputDrift: value.payload.maximumUnexcitedInputDrift }) };
}

function mimoTrial(value: unknown): value is MimoSettledTrial {
  return record(value) && nonempty(value.inputId) && mimoWindow(value.baseline) && mimoWindow(value.settled);
}
function mimoWindow(value: unknown): value is { startSeconds: number; endSeconds: number } {
  return record(value) && finite(value.startSeconds) && finite(value.endSeconds) && value.startSeconds < value.endSeconds;
}

/**
 * The envelope every page message carries, with the checks of `decodeEngineeringCommand`: its exact keys, its schema,
 * its kind and its scope. A message that fails them is not Control Studio's and is left to the other routes.
 */
function pageEnvelope(value: unknown, expected: LoopMessageScope, kind: string): { messageId: string; payload: Record<string, unknown> } | undefined {
  if (!record(value) || !record(value.payload) || JSON.stringify(value).length > 16_000 ||
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

/**
 * The settings whose predicted response the page asks for (B4.5). A value that is not a number is kept as NaN, so the
 * preview refuses it with a reason instead of the request disappearing.
 */
export function decodeManualPreviewRequest(value: unknown, expected: LoopMessageScope): { messageId: string; settings: Record<string, number> } | undefined {
  const envelope = pageEnvelope(value, expected, "control-studio.engineer.manual-preview");
  if (!envelope || Object.keys(envelope.payload).join(",") !== "settings" || !record(envelope.payload.settings)) return undefined;
  const entries = Object.entries(envelope.payload.settings);
  if (entries.length > 64 || entries.some(([symbol]) => !symbol)) return undefined;
  return { messageId: envelope.messageId,
    settings: Object.fromEntries(entries.map(([symbol, number]) => [symbol, typeof number === "number" ? number : Number.NaN])) };
}
