import * as vscode from "vscode";
import { randomUUID } from "crypto";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopRecording } from "./loopMonitor";
import { ControlExperimentClient, identificationRecordForRole, type ExperimentIdentificationRecord } from "./experiment";
import type { IdentificationSample } from "./identification";
import type { WorkflowResultSample } from "./workflowResult";
export { identificationOriginalRecording } from "./identificationState";
import type { ControlPidCandidate } from "./controllerCandidate";
import { candidateFromRuntimeParameters } from "./pidCandidateFactory";
import type { ControllerApplicationClient } from "./controllerApplication";
import type { IdentifiedModel } from "./identification";
import type { ValidationResult } from "./validation";
import { runtimeLifecycleService } from "../runtimeLifecycle";
import { sendRuntimeControlRequest } from "../runtimeControlClient";

export async function postExperimentResult(panel: vscode.WebviewPanel | undefined,
  loop: ControlLoopDefinition | undefined, editorSessionId: string, phase: string, payload: unknown): Promise<void> {
  if (!panel || !loop) return;
  await panel.webview.postMessage({
    schemaVersion: 1, kind: "control-studio.experiment.result", messageId: randomUUID(),
    projectId: loop.projectId, editorSessionId, loopId: loop.id, expectedLoopRevision: loop.revision, phase, payload,
  });
}

export function runtimeExperimentClient(): ControlExperimentClient {
  return new ControlExperimentClient(async (operation, params) => {
    const lifecycle = await runtimeLifecycleService.snapshot();
    if (!lifecycle.status.running) throw new Error("The runtime is not running.");
    const token = await runtimeLifecycleService.controlAuthToken(lifecycle.status.endpoint);
    return sendRuntimeControlRequest(lifecycle.status.endpoint, token, operation, params, { timeoutMs: 2_000 });
  });
}

export function identificationRecordForSource(records: readonly ExperimentIdentificationRecord[],
  role: "estimation" | "validation", selected: LoopRecording | undefined): ExperimentIdentificationRecord | undefined {
  if (!selected) return identificationRecordForRole(records, role);
  const id = selected.evidence.recordingId;
  return selected.provenance || !id ? undefined : records.find(record =>
    record.outcome === "completed" && record.acquisition?.recordingId === id);
}

export function identificationSamplesFromEvidence(testRecord: ExperimentIdentificationRecord | undefined,
  completedRecording: LoopRecording | undefined, retainedSamples: readonly WorkflowResultSample[] | undefined): IdentificationSample[] {
  if (testRecord && !completedRecording?.provenance && (!completedRecording ||
    completedRecording.evidence.recordingId && testRecord.acquisition?.recordingId === completedRecording.evidence.recordingId)) {
    return testRecord.samples.map((sample) => ({ ...sample }));
  }
  const recordedSamples = completedRecording?.samples ?? [];
  const first = recordedSamples[0]?.observedAtMs ?? 0;
  const liveSamples = recordedSamples.map((sample) => ({
    timeSeconds: (sample.observedAtMs - first) / 1_000,
    input: sample.manipulatedVariable.value,
    output: sample.processVariable.value,
    quality: ([sample.manipulatedVariable, sample.processVariable].some((signal) => signal.quality === "forced")
      ? "bad" : "good") as "bad" | "good",
    inputUnit: sample.manipulatedVariable.unit,
    outputUnit: sample.processVariable.unit,
  }));
  if (liveSamples.length > 0) return liveSamples;
  return (completedRecording?.rawSamples ?? retainedSamples)?.map((sample) => ({
    timeSeconds: sample.elapsedSeconds,
    input: sample.appliedMv ?? Number.NaN,
    output: sample.pv ?? Number.NaN,
    quality: sample.quality === "good" ? "good" as const : "bad" as const,
    inputUnit: sample.units.mv,
    outputUnit: sample.units.pv,
  })) ?? [];
}

export function currentStageEvidence(model: IdentifiedModel | undefined, candidate: ControlPidCandidate | undefined,
  validation: ValidationResult | undefined) {
  return {
    hasModel: Boolean(model), hasCandidate: Boolean(candidate), validationHasResult: Boolean(validation),
    validationPassed: validation ? !validation.requirements.some(requirement =>
      requirement.status === "Fail" || requirement.status === "Inconclusive") : undefined,
  };
}

export async function postEngineeringResult(panel: vscode.WebviewPanel | undefined,
  loop: ControlLoopDefinition | undefined, editorSessionId: string, stage: string, payload: unknown): Promise<void> {
  if (!panel || !loop) return;
  const visibleStage = stage === "tune-settings" ? "tune" : stage;
  await panel.webview.postMessage({ schemaVersion: 1, kind: "control-studio.engineering.result", messageId: randomUUID(),
    projectId: loop.projectId, editorSessionId, loopId: loop.id, expectedLoopRevision: loop.revision, stage: visibleStage, payload });
}

export async function postApplicationResult(panel: vscode.WebviewPanel | undefined,
  loop: ControlLoopDefinition | undefined, editorSessionId: string, phase: string, payload: unknown): Promise<void> {
  if (!panel || !loop) return;
  await panel.webview.postMessage({ schemaVersion: 1, kind: "control-studio.application.result", messageId: randomUUID(),
    projectId: loop.projectId, editorSessionId, loopId: loop.id, expectedLoopRevision: loop.revision, phase, payload });
}

export async function currentRuntimeCandidate(loop: ControlLoopDefinition,
  controllerClient: () => Promise<ControllerApplicationClient>): Promise<ControlPidCandidate> {
  return candidateFromRuntimeParameters((await (await controllerClient()).status(loop.controller.instance)).parameters as Record<string, unknown>);
}
