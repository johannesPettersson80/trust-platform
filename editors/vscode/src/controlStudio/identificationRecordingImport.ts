import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMessageScope } from "./messages";
import type { LoopRecording } from "./loopMonitor";
import { decodeWorkflowResult, type WorkflowRecordingProvenance } from "./workflowResult";
import { displayEngineeringUnit } from "./unitDisplay";
import { importedRecordingId } from "./recordingIdentity";
import { identificationOriginalRecording, type SavedIdentificationInput } from "./identificationState";
export { importedRecordingId };

export const MAX_RECORDING_IMPORT_BYTES = 32 * 1024 * 1024;

/** Import measured data and original identity without adopting foreign design approval. */
export function importIdentificationRecording(json: string, loop: ControlLoopDefinition): LoopRecording {
  return readIdentificationRecordingImport(json, loop).recording;
}

export function readIdentificationRecordingImport(json: string, loop: ControlLoopDefinition): {
  recording: LoopRecording; reference?: SavedIdentificationInput;
} {
  if (json.length > MAX_RECORDING_IMPORT_BYTES) throw new Error("Recording file exceeds 32 MiB.");
  let value: unknown;
  try { value = JSON.parse(json); } catch { throw new Error("Choose a valid Control Studio result JSON file."); }
  const result = decodeWorkflowResult(value);
  if (!result) throw new Error("The saved recording has invalid data or source metadata.");
  const estimation = result.identificationStudy?.estimation;
  const reference = estimation?.requestedKind === "fopdt" && estimation.stepResponseSettings && estimation.recording ? estimation : undefined;
  const data = reference?.recording?.data ?? result.recording;
  const acquisition = data.acquisition;
  const samples = data.rawSamples;
  if (!acquisition || acquisition.method !== "runtime_scan_buffer" || acquisition.completeness !== "complete" ||
      acquisition.missingScanRanges.length || acquisition.droppedObservedSamples || !acquisition.actualCycleTimingAvailable ||
      samples.length < 2 || samples.length > 12000 || acquisition.observedSampleCount !== samples.length ||
      acquisition.scanRange?.first !== samples[0].scan || acquisition.scanRange?.last !== samples[samples.length - 1].scan ||
      samples.some((sample, i) => i > 0 && sample.scan !== samples[i - 1].scan + 1)) {
    throw new Error("Identify requires a complete recording of 2–12000 consecutive runtime scans.");
  }
  const provenance: WorkflowRecordingProvenance = data.provenance ?? {
    schemaVersion: 1, resultId: result.resultId, resultRevision: result.resultRevision, createdAt: result.createdAt,
    projectId: result.projectId, source: result.source, loop: result.loop, controller: result.controller,
    software: result.software, units: result.units,
    ...(!reference && result.plant ? { plant: result.plant } : {}), ...(!reference && result.experiment ? { experiment: result.experiment } : {}),
  };
  if (provenance.experiment && provenance.experiment.status !== "completed") {
    throw new Error("An aborted or failed experiment cannot supply an Identify recording.");
  }
  if (displayEngineeringUnit(provenance.units.pv) !== displayEngineeringUnit(loop.signals.processVariable.unit) ||
      displayEngineeringUnit(provenance.units.mv) !== displayEngineeringUnit(loop.signals.manipulatedVariable.unit)) {
    throw new Error("Recording input and output units must match this loop; no unit conversion was applied.");
  }
  const recording: LoopRecording = { samples: [], rawSamples: samples, evidence: acquisition, provenance };
  return freeze({ recording, ...(reference ? { reference: { ...reference,
    estimationRecordingId: importedRecordingId(provenance),
    recording: identificationOriginalRecording(recording, reference.recording!.kind),
  } } : {}) });
}

/** Only this scoped file-import message can select foreign data. It has no runtime client. */
export async function routeIdentificationRecordingImport(message: unknown, scope: LoopMessageScope,
  loop: ControlLoopDefinition, busy: boolean, receive: (recording: LoopRecording, reference?: SavedIdentificationInput) => Promise<void>,
  post: (message: Record<string, unknown>) => Promise<unknown>): Promise<boolean> {
  if (!record(message) || message.kind !== "control-studio.identify.import") return false;
  if (message.schemaVersion !== 1 || message.projectId !== scope.projectId || message.loopId !== scope.loopId ||
      message.editorSessionId !== scope.editorSessionId || message.expectedLoopRevision !== scope.loopRevision ||
      typeof message.messageId !== "string" || !message.messageId || !record(message.payload) ||
      Object.keys(message.payload).join(",") !== "json" || typeof message.payload.json !== "string") return true;
  const respond = (payload: Record<string, unknown>) => post({ schemaVersion: 1,
    kind: "control-studio.identification-recording", messageId: message.messageId,
    projectId: scope.projectId, loopId: scope.loopId, editorSessionId: scope.editorSessionId,
    expectedLoopRevision: scope.loopRevision, payload });
  try {
    if (busy) throw new Error("Stop the active recording before importing another recording.");
    const imported = readIdentificationRecordingImport(message.payload.json, loop);
    await receive(imported.recording, imported.reference);
    await respond({ status: "imported", ...imported });
  } catch (error) {
    await respond({ status: "failed", reason: error instanceof Error ? error.message : String(error) });
  }
  return true;
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function freeze<T>(value: T): T {
  if (value && typeof value === "object") { Object.values(value).forEach(freeze); Object.freeze(value); }
  return value;
}
