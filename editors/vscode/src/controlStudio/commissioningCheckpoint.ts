import { decodeGeneralModel } from "./generalModel";
import { engineerRequirements } from "./engineerVerificationPlan";
import type { LoopRecording } from "./loopMonitor";
import { randomUUID } from "crypto";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { ParameterOwner, ParameterStatus } from "./parameterApplication";
import type { TransferBinding } from "./proportionalTransfer";
import type { TransferOwner, TransferTracking } from "./runtimeTransfer";
import type { RecordingRecoveryOwner, RecordingOwnerPersistence } from "./runtimeRecording";
import type { CommissioningResult } from "./proportionalCommissioning";
import { RECORDING_SAMPLE_LIMIT } from "./controlStudioLimits";

/** Recovery data cannot recreate a review token or authorize Auto entry. */
export interface CommissioningCheckpoint {
  id: string;
  loopRevision: string;
  stage: "prepared" | "recording" | "parameters" | "transfer" | "completed" | "recovered" | "retired";
  model: GeneralModel;
  task: EngineerRequirements;
  origin: ParameterStatus;
  candidate: Readonly<Record<string, number>>;
  parameterOperation: ParameterOwner;
  references: Readonly<Record<string, number>>;
  binding: TransferBinding;
  recoveryTracking: TransferTracking;
  tolerance: number;
  replacementProgramRevision?: string;
  transfer?: TransferOwner;
  recording?: RecordingRecoveryOwner;
  recordingResult?: LoopRecording;
  /** The mode the loop was found in at review (A6.2). A checkpoint written before this field has none, and its restore stays in Manual. */
  foundMode?: "auto" | "manual";
  /** The measured output's noise Apply's Auto entry allowed for (A6.4), so that a restore's return to Auto allows the same. */
  processNoise?: number;
}
export type CommissioningCheckpointWriter = (checkpoint: CommissioningCheckpoint) => Promise<void>;

/** Keeps a completed receipt available for inspection only when it is the receipt
 * produced by this durable checkpoint; unresolved or retired checkpoints never
 * restore a commissioning result as authority. */
export function completedCommissioningMatchesCheckpoint(receipt: CommissioningResult | undefined,
  checkpoint: CommissioningCheckpoint): boolean {
  if (!receipt || checkpoint.stage !== "completed" || receipt.status !== "passed" || !receipt.recoveryConfirmed
    || receipt.modelRevision !== checkpoint.model.revision || !checkpoint.transfer
    || !receipt.parameters || receipt.parameters.program_revision !== checkpoint.origin.program_revision
    || receipt.parameters.binding_revision !== checkpoint.origin.binding_revision
    || receipt.parameters.transaction_id !== checkpoint.parameterOperation.id
    || !sameNumericRecord(receipt.parameters.values, checkpoint.candidate)) return false;
  const owner = checkpoint.transfer;
  return receipt.transfers.some(transfer => transfer.transaction?.id === owner.id
    && transfer.transaction.program_revision === owner.program
    && transfer.transaction.binding_revision === owner.binding
    && transfer.transaction.confirmation === "output-image");
}

export class CommissioningJournal {
  private checkpoint: CommissioningCheckpoint;
  private writes: Promise<void> = Promise.resolve();
  constructor(loop: ControlLoopDefinition, data: Omit<CommissioningCheckpoint, "id" | "loopRevision" | "stage">,
    private readonly writer: CommissioningCheckpointWriter) {
    this.checkpoint = { ...structuredClone(data), id: randomUUID(), loopRevision: loop.revision, stage: "prepared" };
  }
  save(stage: CommissioningCheckpoint["stage"], update: Partial<Pick<CommissioningCheckpoint, "transfer" | "recording" | "recordingResult" | "parameterOperation">> = {}): Promise<void> {
    this.checkpoint = { ...this.checkpoint, ...structuredClone(update), stage };
    const snapshot = structuredClone(this.checkpoint);
    this.writes = this.writes.catch(() => undefined).then(() => this.writer(snapshot));
    return this.writes;
  }
  recordingPersistence(): RecordingOwnerPersistence {
    return { persist: owner => this.save("recording", { recording: owner }),
      clear: () => this.save("prepared", { recording: undefined }) };
  }
}

export function decodeCommissioningCheckpoint(value: unknown, loop: ControlLoopDefinition): CommissioningCheckpoint | undefined {
  if (!record(value) || !text(value.id) || value.loopRevision !== loop.revision
    || !["prepared", "recording", "parameters", "transfer", "completed", "recovered", "retired"].includes(String(value.stage))
    || JSON.stringify(value).length > 16 * 1024 * 1024 || !decodeGeneralModel(value.model) || !record(value.task)
    || !record(value.origin) || ![value.origin.program_revision, value.origin.binding_revision, value.origin.active_revision].every(text)
    || (value.origin.supports_request_id !== undefined && typeof value.origin.supports_request_id !== "boolean")
    || (value.origin.transaction_id !== undefined && value.origin.transaction_id !== null && !text(value.origin.transaction_id))
    || !record(value.binding) || value.binding.loop_id !== loop.id || !record(value.recoveryTracking)
    || !record(value.references) || !Object.keys(value.references).length
    || !Object.values(value.references).every(number) || !number(value.tolerance) || value.tolerance < 0) return undefined;
  const validValues = (values: unknown) => record(values) && !!loop.parameterBinding
    && Object.keys(values).length === loop.parameterBinding.parameters.length
    && loop.parameterBinding.parameters.every(parameter => {
      const candidate = values[parameter.symbol];
      return number(candidate) && candidate >= parameter.minimum && candidate <= parameter.maximum
        && (parameter.iec_type !== "REAL" || Math.fround(candidate) === candidate);
    });
  if (!validValues(value.origin.values) || !validValues(value.candidate)
    || !(value.origin.previous_revision === null || text(value.origin.previous_revision))
    || !(value.origin.previous_values === null || validValues(value.origin.previous_values))) return undefined;
  if (!record(value.parameterOperation) || !text(value.parameterOperation.id)
    || value.parameterOperation.program !== value.origin.program_revision || value.parameterOperation.binding !== value.origin.binding_revision) return undefined;
  if (value.transfer !== undefined && (!record(value.transfer) || !text(value.transfer.id)
    || value.transfer.program !== value.origin.program_revision || !text(value.transfer.binding))) return undefined;
  if (value.recording !== undefined && (!record(value.recording) || value.recording.loopRevision !== loop.revision
    || value.recording.loopId !== loop.id || value.recording.projectId !== loop.projectId || value.recording.runtimeId !== loop.runtimeId
    || !text(value.recording.recordingId) || !Array.isArray(value.recording.bindings))) return undefined;
  if (value.recordingResult !== undefined && !validRecordingResult(value.recordingResult)) return undefined;
  if (value.foundMode !== undefined && !["auto", "manual"].includes(String(value.foundMode))) return undefined;
  if (value.processNoise !== undefined && (!number(value.processNoise) || value.processNoise < 0)) return undefined;
  try { engineerRequirements(value.task); } catch { return undefined; }
  return structuredClone(value) as unknown as CommissioningCheckpoint;
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function text(value: unknown): value is string { return typeof value === "string" && !!value; }
function number(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function sameNumericRecord(left: Readonly<Record<string, number>>, right: Readonly<Record<string, number>>): boolean {
  return Object.keys(left).length === Object.keys(right).length && Object.keys(right).every(symbol => left[symbol] === right[symbol]);
}

function validRecordingResult(value: unknown): boolean {
  if (!record(value) || !Array.isArray(value.samples) || value.samples.length > RECORDING_SAMPLE_LIMIT || !record(value.evidence)) return false;
  const rawSamples = value.rawSamples;
  if (rawSamples !== undefined && (!Array.isArray(rawSamples) || rawSamples.length > RECORDING_SAMPLE_LIMIT)) return false;
  const sampleCount = rawSamples !== undefined ? rawSamples.length : value.samples.length;
  // Runtime recordings retain their decoded samples in rawSamples and leave
  // the monitor projection empty; UI recordings use samples directly.
  const evidence = value.evidence;
  const observedSampleCount = evidence.observedSampleCount;
  const droppedObservedSamples = evidence.droppedObservedSamples;
  if (!["ui_observed_snapshots", "runtime_scan_buffer"].includes(String(evidence.method))
    || !["incomplete", "complete"].includes(String(evidence.completeness))
    || !number(observedSampleCount) || !Number.isSafeInteger(observedSampleCount) || observedSampleCount < 0
    || observedSampleCount !== sampleCount || !Array.isArray(evidence.missingScanRanges)
    || evidence.missingScanRanges.length > RECORDING_SAMPLE_LIMIT || !evidence.missingScanRanges.every(range => {
      if (!record(range)) return false;
      const first = range.first, last = range.last;
      return number(first) && number(last) && Number.isSafeInteger(first) && Number.isSafeInteger(last) && first <= last;
    })
    || !number(droppedObservedSamples) || !Number.isSafeInteger(droppedObservedSamples) || droppedObservedSamples < 0
    || typeof evidence.actualCycleTimingAvailable !== "boolean" || typeof evidence.freshnessAvailable !== "boolean"
    || typeof evidence.reason !== "string") return false;
  return true;
}
