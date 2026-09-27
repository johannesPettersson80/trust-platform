import type { ControlLoopDefinition } from "./loopDefinition";
import {
  identificationSamplesFromRuntimeRecording,
  retainedIdentificationSamplesFromRuntimeRecording,
  type ControlExperimentPlan,
  type ExperimentIdentificationRecord,
  type ExperimentIdentificationSample,
  type ExperimentIdentificationTerminal,
} from "./experiment";
import type { LoopRecording, LoopRecordingEvidence } from "./loopMonitor";
import { assessConfiguredSaturationWindup } from "./saturationWindup";

export interface TerminalTestEvidenceInput {
  readonly loop: ControlLoopDefinition | undefined;
  readonly plan: ControlExperimentPlan | undefined;
  readonly result: unknown;
  readonly stopRecording: () => Promise<LoopRecording>;
  readonly recordIdentification: (
    plan: ControlExperimentPlan,
    terminal: ExperimentIdentificationTerminal,
    samples: readonly ExperimentIdentificationSample[],
    acquisition: LoopRecordingEvidence,
  ) => ExperimentIdentificationRecord;
}

/** Builds immutable terminal Test evidence before any later Identify/Tune consumer can use it. */
export async function buildTerminalTestEvidence(input: TerminalTestEvidenceInput): Promise<unknown> {
  if (!input.plan || !input.loop || !record(input.result)) return input.result;
  const terminal = terminalResult(input.plan, input.result);
  if (!terminal) return input.result;
  const recording = await input.stopRecording();
  let samples: ExperimentIdentificationSample[];
  try {
    samples = identificationSamplesFromRuntimeRecording(input.plan, recording);
  } catch {
    samples = retainedIdentificationSamplesFromRuntimeRecording(input.plan, recording);
  }
  const identification = input.recordIdentification(input.plan, terminal, samples, recording.evidence);
  const saturationWindup = assessConfiguredSaturationWindup(input.loop, recording);
  return {
    ...input.result,
    identification,
    recordingEvidence: recording.evidence,
    recording,
    ...(saturationWindup.status === "unavailable" ? {} : { saturationWindup }),
  };
}

function terminalResult(plan: ControlExperimentPlan, value: Record<string, unknown>): ExperimentIdentificationTerminal | undefined {
  const status = value.status;
  const restoration = value.restoration;
  if (value.experiment_id !== plan.id ||
      !["completed", "aborted", "failed", "failed_to_restore"].includes(String(status)) ||
      !["restored", "not_required", "failed"].includes(String(restoration))) return undefined;
  return {
    experiment_id: value.experiment_id,
    status: status as ExperimentIdentificationTerminal["status"],
    restoration: restoration as ExperimentIdentificationTerminal["restoration"],
    ...(typeof value.stop_reason === "string" || value.stop_reason === null ? { stop_reason: value.stop_reason } : {}),
    ...(typeof value.restoration_reason === "string" || value.restoration_reason === null
      ? { restoration_reason: value.restoration_reason } : {}),
  };
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
