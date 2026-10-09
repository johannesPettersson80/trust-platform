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
import { runtimeLifecycleService } from "../runtimeLifecycle";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import { getSelectedRuntimeId } from "../selectedRuntime";
import { MainWorkflowError } from "./mainWorkflowFailure";

export async function postExperimentResult(panel: vscode.WebviewPanel | undefined,
  loop: ControlLoopDefinition | undefined, editorSessionId: string, phase: string, payload: unknown): Promise<void> {
  if (!panel || !loop) return;
  await panel.webview.postMessage({
    schemaVersion: 1, kind: "control-studio.experiment.result", messageId: randomUUID(),
    projectId: loop.projectId, editorSessionId, loopId: loop.id, expectedLoopRevision: loop.revision, phase, payload,
  });
}

/** The runtime a study is bound to (A2.7): resolved once, when the loop is opened or reconnected, and kept with the study session. */
export interface StudyRuntimeTarget { readonly runtimeId: string; readonly endpoint: string }

/** Resolves the study's runtime from the current selection. Only opening or reconnecting a loop calls it, never a live request. */
export async function resolveStudyRuntimeTarget(loop: ControlLoopDefinition): Promise<StudyRuntimeTarget> {
  assertSelectedRuntimeIsTarget(loop.runtimeId, getSelectedRuntimeId());
  const lifecycle = await runtimeLifecycleService.snapshot();
  if (!lifecycle.status.running) {
    throw new MainWorkflowError("Start the runtime before running a live action.", { cause: "runtime", nextAction: "connect-runtime" });
  }
  return { runtimeId: loop.runtimeId, endpoint: lifecycle.status.endpoint };
}

/** A live action refuses when the selected runtime is not the study's target: a selection change never switches the study (A2.7). */
export function assertSelectedRuntimeIsTarget(targetRuntimeId: string, selectedRuntimeId: string): void {
  if (selectedRuntimeId === targetRuntimeId) return;
  throw new MainWorkflowError(`Control Studio is connected to ${targetRuntimeId}; the selected runtime is ${selectedRuntimeId || "none"}. Reconnect to continue.`,
    { cause: "runtime", nextAction: "connect-runtime" });
}

/** Live experiment requests go to the study's pinned endpoint, and each one is refused while another runtime is selected. */
export function runtimeExperimentClient(target: StudyRuntimeTarget): ControlExperimentClient {
  return new ControlExperimentClient(async (operation, params) => {
    assertSelectedRuntimeIsTarget(target.runtimeId, getSelectedRuntimeId());
    const token = await runtimeLifecycleService.controlAuthToken(target.endpoint);
    return sendRuntimeControlRequest(target.endpoint, token, operation, params, { timeoutMs: 2_000 });
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

export async function postEngineeringResult(panel: vscode.WebviewPanel | undefined,
  loop: ControlLoopDefinition | undefined, editorSessionId: string, stage: string, payload: unknown): Promise<void> {
  if (!panel || !loop) return;
  await panel.webview.postMessage({ schemaVersion: 1, kind: "control-studio.engineering.result", messageId: randomUUID(),
    projectId: loop.projectId, editorSessionId, loopId: loop.id, expectedLoopRevision: loop.revision, stage, payload });
}

export async function currentRuntimeCandidate(loop: ControlLoopDefinition,
  controllerClient: () => Promise<ControllerApplicationClient>): Promise<ControlPidCandidate> {
  return candidateFromRuntimeParameters((await (await controllerClient()).status(loop.controller.instance)).parameters as Record<string, unknown>);
}
