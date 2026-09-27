import type { PlantControlStudioHandoff } from "./handoff";
import type { PlantMonitorSnapshot } from "./plantMonitor";
import type { AutomaticTuneObjective, AutomaticTuneResult } from "./automaticTune";
import type { LoopMonitorSample, LoopRecording } from "./loopMonitor";
import type { RecordingUpdate } from "./recordingCoordinator";
import type { ControlPidParameters } from "./controllerCandidate";
import { validateControlPidSettings } from "./pidSettingsValidation";
import type { ExcludedInterval } from "./identification";
import type { ControlExperimentRequestKind } from "./experiment";
import { decodeControlChartSession, type ControlChartSession } from "./chartContract";
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

export type AutomaticTuneResultEnvelope = ControlStudioEnvelope<{
  status: "complete" | "failed";
  result?: AutomaticTuneResult;
  error?: string;
  failedStage?: "Test" | "Identify" | "Tune" | "Validate";
  completedStages?: readonly ("Test" | "Identify" | "Tune" | "Validate")[];
  recovery?: string;
}> & { kind: "control-studio.automatic.result"; plantId: string; expectedPlantRevision: string };

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

export function decodeLoopLiveViewCommand(value: unknown, expected: LoopMessageScope): boolean {
  return record(value) && value.kind === "control-studio.monitor.live" &&
    Object.keys(value).sort().join(",") === "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" &&
    value.schemaVersion === 1 && nonempty(value.messageId) && value.projectId === expected.projectId &&
    value.editorSessionId === expected.editorSessionId && value.loopId === expected.loopId &&
    value.expectedLoopRevision === expected.loopRevision && record(value.payload) && Object.keys(value.payload).length === 0;
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

export function decodeControllerApplicationCommand(value: unknown, expected: LoopMessageScope):
  { action: "apply"; messageId: string; physicalApproved: boolean } |
  { action: "rollback"; messageId: string } | undefined {
  if (!record(value) || JSON.stringify(value).length > 16_000 || !record(value.payload) ||
      Object.keys(value).sort().join(",") !== "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" ||
      value.schemaVersion !== 1 || !nonempty(value.messageId) || value.projectId !== expected.projectId ||
      value.editorSessionId !== expected.editorSessionId || value.loopId !== expected.loopId ||
      value.expectedLoopRevision !== expected.loopRevision) return undefined;
  if (value.kind === "control-studio.candidate.apply" &&
      Object.keys(value.payload).join(",") === "physicalApproved" && typeof value.payload.physicalApproved === "boolean") {
    return { action: "apply", messageId: value.messageId, physicalApproved: value.payload.physicalApproved };
  }
  if (value.kind === "control-studio.candidate.rollback" && Object.keys(value.payload).length === 0) {
    return { action: "rollback", messageId: value.messageId };
  }
  return undefined;
}

export type WorkflowResultCommand =
  | { action: "save"; messageId: string; session?: ControlChartSession }
  | { action: "export-csv" | "export-json"; messageId: string }
  | { action: "open"; messageId: string; resultRevision: number }
  | { action: "compare"; messageId: string; leftRevision: number; rightRevision: number };

export function decodeWorkflowResultCommand(value: unknown, expected: LoopMessageScope): WorkflowResultCommand | undefined {
  if (!record(value) || !record(value.payload) || JSON.stringify(value).length > 16_000 ||
      Object.keys(value).sort().join(",") !== "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" ||
      value.schemaVersion !== 1 || !nonempty(value.messageId) || value.projectId !== expected.projectId ||
      value.editorSessionId !== expected.editorSessionId || value.loopId !== expected.loopId ||
      value.expectedLoopRevision !== expected.loopRevision) return undefined;
  if (Object.keys(value.payload).length === 0) {
    if (value.kind === "control-studio.result.save") return { action: "save", messageId: value.messageId };
    if (value.kind === "control-studio.result.export-csv") return { action: "export-csv", messageId: value.messageId };
    if (value.kind === "control-studio.result.export-json") return { action: "export-json", messageId: value.messageId };
  }
  if (value.kind === "control-studio.result.save" &&
      Object.keys(value.payload).join(",") === "session") {
    const session = decodeControlChartSession(value.payload.session,
      `${expected.loopId}:${expected.loopRevision}`);
    if (session) return { action: "save", messageId: value.messageId, session };
  }
  if (value.kind === "control-studio.result.open" &&
      Object.keys(value.payload).join(",") === "resultRevision" && positiveInteger(value.payload.resultRevision)) {
    return { action: "open", messageId: value.messageId, resultRevision: value.payload.resultRevision };
  }
  if (value.kind === "control-studio.result.compare" &&
      Object.keys(value.payload).sort().join(",") === "leftRevision,rightRevision" &&
      positiveInteger(value.payload.leftRevision) && positiveInteger(value.payload.rightRevision) &&
      value.payload.leftRevision !== value.payload.rightRevision) {
    return { action: "compare", messageId: value.messageId,
      leftRevision: value.payload.leftRevision, rightRevision: value.payload.rightRevision };
  }
  return undefined;
}

export function decodeLoopAutomaticTuneApprove(value: unknown, expected: LoopMessageScope):
  { messageId: string; objective: AutomaticTuneObjective } | undefined {
  if (!record(value) || !record(value.payload) || JSON.stringify(value).length > 16_000 ||
      Object.keys(value).sort().join(",") !== "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" ||
      value.schemaVersion !== 1 || value.kind !== "control-studio.automatic.approve" || !nonempty(value.messageId) ||
      value.projectId !== expected.projectId || value.editorSessionId !== expected.editorSessionId ||
      value.loopId !== expected.loopId || value.expectedLoopRevision !== expected.loopRevision ||
      Object.keys(value.payload).join(",") !== "objective" ||
      !["balanced", "robust", "fast"].includes(String(value.payload.objective))) return undefined;
  return { messageId: value.messageId, objective: value.payload.objective as AutomaticTuneObjective };
}

import { validStepResponseSettings, type StepResponseSettings } from "./stepResponseIdentification";
export type EngineeringCommand =
  | { action: "identify-choose-structure"; messageId: string; choice: import("./sensorLagComparison").StructureChoice }
  | { action: "declared-root"; messageId: string; selectedValue?: number }
  | { action: "identify-edit"; messageId: string; form: IdentificationFormDraft }
  | { action: "identify-qualify"; messageId: string; model: string; excludedIntervals?: readonly ExcludedInterval[]; stepResponseSettings?: StepResponseSettings }
  | { action: "identify-fit" | "identify-accept" | "identify-cancel"; messageId: string }
  | { action: "tune"; messageId: string; method: "SIMC" | "AMIGO"; objective: AutomaticTuneObjective }
  | { action: "tune-settings"; messageId: string; method: "Manual" | "Imported"; settings: ControlPidParameters }
  | { action: "validate"; messageId: string };
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
  if (value.kind === "control-studio.tune.run" && Object.keys(value.payload).sort().join(",") === "method,objective" &&
      (value.payload.method === "SIMC" || value.payload.method === "AMIGO") &&
      ["balanced", "robust", "fast"].includes(String(value.payload.objective))) {
    return { action: "tune", messageId: value.messageId, method: value.payload.method,
      objective: value.payload.objective as AutomaticTuneObjective };
  }
  if (value.kind === "control-studio.tune.run" && Object.keys(value.payload).sort().join(",") === "method,settings" &&
      (value.payload.method === "Manual" || value.payload.method === "Imported")) {
    const settings = validateControlPidSettings(value.payload.settings);
    if (!settings.ok) return undefined;
    return { action: "tune-settings", messageId: value.messageId, method: value.payload.method,
      settings: settings.value };
  }
  if (value.kind === "control-studio.validate.run" && Object.keys(value.payload).length === 0) {
    return { action: "validate", messageId: value.messageId };
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

function positiveInteger(value: unknown): value is number {
  return Number.isInteger(value) && (value as number) > 0;
}

const PLANT_MONITOR_ENVELOPE_KEYS = new Set([
  "schemaVersion", "kind", "messageId", "projectId", "editorSessionId",
  "loopId", "plantId", "expectedPlantRevision", "payload",
]);
const AVAILABLE_PLANT_MONITOR_KEYS = new Set([
  "availability", "source", "plantId", "activeConfigurationRevision",
  "runtimeVersion", "simulationTimeNs", "operatingState", "model", "command",
  "appliedInput", "idealOutput", "measuredFeedback", "publishedFeedback",
  "state", "timeScale", "effectiveSeed", "randomAlgorithm",
  "randomAlgorithmVersion", "actuators", "activeUncertainties",
  "activeDropouts", "pendingEventCount", "delayBufferDepth", "lastSubsteps",
  "currentFault",
]);

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

export function decodeLoopRecordingCommand(
  value: unknown,
  expected: LoopMessageScope
): { action: "start" | "stop"; messageId: string } | undefined {
  if (!record(value) || JSON.stringify(value).length > 16_000) return undefined;
  const keys = Object.keys(value).sort().join(",");
  if (
    keys !== "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" ||
    value.schemaVersion !== 1 || !nonempty(value.messageId) ||
    value.projectId !== expected.projectId || value.editorSessionId !== expected.editorSessionId ||
    value.loopId !== expected.loopId || value.expectedLoopRevision !== expected.loopRevision ||
    !record(value.payload) || Object.keys(value.payload).length !== 0
  ) return undefined;
  if (value.kind === "control-studio.recording.start") {
    return { action: "start", messageId: value.messageId };
  }
  if (value.kind === "control-studio.recording.stop") {
    return { action: "stop", messageId: value.messageId };
  }
  return undefined;
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
  | { action: "abort"; messageId: string; experimentId: string }
  | { action: "calculate-relay"; messageId: string };

export type AntiWindupCommand =
  | { action: "review"; messageId: string; trackingTimeSeconds: number }
  | { action: "stage" | "verify" | "rollback"; messageId: string };

export function decodeAntiWindupCommand(value: unknown, expected: LoopMessageScope): AntiWindupCommand | undefined {
  if (!record(value) || JSON.stringify(value).length > 16_000 || !record(value.payload) ||
      Object.keys(value).sort().join(",") !== "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" ||
      value.schemaVersion !== 1 || !nonempty(value.messageId) || value.projectId !== expected.projectId ||
      value.editorSessionId !== expected.editorSessionId || value.loopId !== expected.loopId || value.expectedLoopRevision !== expected.loopRevision) return undefined;
  if (value.kind === "control-studio.anti-windup.review" && Object.keys(value.payload).join(",") === "trackingTimeSeconds" &&
      finite(value.payload.trackingTimeSeconds) && value.payload.trackingTimeSeconds > 0) {
    return { action: "review", messageId: value.messageId, trackingTimeSeconds: value.payload.trackingTimeSeconds };
  }
  if ((value.kind === "control-studio.anti-windup.stage" || value.kind === "control-studio.anti-windup.verify" || value.kind === "control-studio.anti-windup.rollback") &&
      Object.keys(value.payload).length === 0) {
    return { action: value.kind.endsWith("stage") ? "stage" : value.kind.endsWith("verify") ? "verify" : "rollback", messageId: value.messageId };
  }
  return undefined;
}

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
  if (value.kind === "control-studio.relay.calculate" && Object.keys(payload).length === 0) {
    return { action: "calculate-relay", messageId: value.messageId };
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

export function decodePlantMonitorEnvelope(
  value: unknown,
  expected: {
    projectId: string;
    editorSessionId: string;
    plantId: string;
    plantRevision: string;
  }
): PlantMonitorEnvelope | undefined {
  if (!record(value) || JSON.stringify(value).length > 256_000) return undefined;
  if (
    Object.keys(value).some((key) => !PLANT_MONITOR_ENVELOPE_KEYS.has(key)) ||
    value.schemaVersion !== 1 ||
    value.kind !== "plant.monitor.snapshot" ||
    !nonempty(value.messageId) ||
    value.projectId !== expected.projectId ||
    value.editorSessionId !== expected.editorSessionId ||
    value.plantId !== expected.plantId ||
    value.expectedPlantRevision !== expected.plantRevision ||
    !plantMonitorSnapshot(value.payload) ||
    value.payload.plantId !== expected.plantId
  ) return undefined;
  return value as unknown as PlantMonitorEnvelope;
}

export function decodeControlStudioReady(
  value: unknown,
  expected: {
    projectId: string;
    editorSessionId: string;
    plantId: string;
    plantRevision: string;
  }
): boolean {
  if (!record(value) || JSON.stringify(value).length > 16_000) return false;
  const keys = Object.keys(value).sort().join(",");
  return keys === "editorSessionId,expectedPlantRevision,kind,messageId,payload,plantId,projectId,schemaVersion" &&
    value.schemaVersion === 1 &&
    value.kind === "control-studio.ready" &&
    nonempty(value.messageId) &&
    value.projectId === expected.projectId &&
    value.editorSessionId === expected.editorSessionId &&
    value.plantId === expected.plantId &&
    value.expectedPlantRevision === expected.plantRevision &&
    record(value.payload) &&
    Object.keys(value.payload).length === 0;
}

export function decodeControlStudioNavigatePlant(
  value: unknown,
  expected: {
    projectId: string;
    editorSessionId: string;
    plantId: string;
    plantRevision: string;
  }
): boolean {
  if (!record(value) || JSON.stringify(value).length > 16_000) return false;
  const message = value as Record<string, unknown>;
  return Object.keys(message).sort().join(",") ===
    "editorSessionId,expectedPlantRevision,kind,messageId,payload,plantId,projectId,schemaVersion" &&
    message.schemaVersion === 1 &&
    message.kind === "control-studio.navigate-plant" &&
    nonempty(message.messageId) &&
    message.projectId === expected.projectId &&
    message.editorSessionId === expected.editorSessionId &&
    message.plantId === expected.plantId &&
    message.expectedPlantRevision === expected.plantRevision &&
    record(message.payload) && Object.keys(message.payload).length === 0;
}

export function decodeAutomaticTuneApprove(
  value: unknown,
  expected: { projectId: string; editorSessionId: string; plantId: string; plantRevision: string }
): { messageId: string; objective: AutomaticTuneObjective } | undefined {
  if (!record(value) || JSON.stringify(value).length > 16_000) return undefined;
  const hasPayload = record(value.payload);
  const payload = value.payload as Record<string, unknown>;
  if (
    value.schemaVersion !== 1 || value.kind !== "control-studio.automatic.approve" ||
    !nonempty(value.messageId) || value.projectId !== expected.projectId ||
    value.editorSessionId !== expected.editorSessionId || value.plantId !== expected.plantId ||
    value.expectedPlantRevision !== expected.plantRevision || !hasPayload ||
    Object.keys(payload).sort().join(",") !== "objective" ||
    !["balanced", "robust", "fast"].includes(String(payload.objective))
  ) return undefined;
  return { messageId: value.messageId, objective: payload.objective as AutomaticTuneObjective };
}

export function automaticTuneResultEnvelope(
  handoff: PlantControlStudioHandoff,
  editorSessionId: string,
  messageId: string,
  payload: AutomaticTuneResultEnvelope["payload"]
): AutomaticTuneResultEnvelope {
  return {
    schemaVersion: 1,
    kind: "control-studio.automatic.result",
    messageId,
    projectId: handoff.projectId,
    editorSessionId,
    ...(handoff.loopId ? { loopId: handoff.loopId } : {}),
    plantId: handoff.plantId,
    expectedPlantRevision: handoff.savedConfigurationRevision,
    payload,
  };
}

function plantMonitorSnapshot(value: unknown): value is PlantMonitorSnapshot {
  if (!record(value) || value.source !== "Simulation" || value.plantId === undefined) return false;
  if (value.availability === "unavailable") {
    return nonempty(value.reason) && Object.keys(value).every((key) =>
      ["availability", "source", "plantId", "reason"].includes(key));
  }
  return value.availability === "available" &&
    Object.keys(value).length === AVAILABLE_PLANT_MONITOR_KEYS.size &&
    Object.keys(value).every((key) => AVAILABLE_PLANT_MONITOR_KEYS.has(key)) &&
    revision(value.activeConfigurationRevision) &&
    nonempty(value.runtimeVersion) &&
    unsignedDecimal(value.simulationTimeNs) &&
    (value.operatingState === "running" || value.operatingState === "faulted") &&
    nonempty(value.model) &&
    numericVector(value.command) &&
    numericVector(value.appliedInput) &&
    numericVector(value.idealOutput) &&
    numericVector(value.measuredFeedback) &&
    numericVector(value.publishedFeedback) &&
    numericVector(value.state) &&
    finite(value.timeScale) && value.timeScale > 0 &&
    unsignedDecimal(value.effectiveSeed) &&
    nonempty(value.randomAlgorithm) &&
    nonnegativeInteger(value.randomAlgorithmVersion) &&
    Array.isArray(value.actuators) &&
    stringArray(value.activeUncertainties) &&
    stringArray(value.activeDropouts) &&
    nonnegativeInteger(value.pendingEventCount) &&
    nonnegativeInteger(value.delayBufferDepth) &&
    nonnegativeInteger(value.lastSubsteps) &&
    (value.currentFault === null || nonempty(value.currentFault));
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

function numericVector(value: unknown): value is number[] {
  return Array.isArray(value) && value.every(finite);
}

function stringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === "string");
}

function nonnegativeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function unsignedDecimal(value: unknown): value is string {
  return typeof value === "string" && /^(0|[1-9][0-9]*)$/.test(value);
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
