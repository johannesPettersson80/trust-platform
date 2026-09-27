import type { LoopRecording } from "./loopMonitor";
import { saturationWindupEvidence, type ControlLoopDefinition } from "./loopDefinition";
import type { WorkflowResultSample } from "./workflowResult";

export interface SaturationWindupAssessmentInput {
  readonly outputMinimum: number;
  readonly outputMaximum: number;
  readonly requestedOutputId: string;
  readonly integralStateId: string;
  readonly recording: Pick<LoopRecording, "rawSamples" | "evidence">;
}

export type SaturationWindupAssessment =
  | {
    readonly status: "qualified";
    readonly limit: "upper" | "lower";
    readonly enteredAtSeconds: number;
    readonly targetReturnAtSeconds: number;
    readonly releasedAtSeconds: number;
    readonly recoverySeconds: number;
    readonly peakRequestedOutput: number;
    readonly peakAppliedOutput: number;
    readonly maximumMismatch: number;
    readonly integralAtEntry: number;
    readonly integralAtTargetReturn: number;
    readonly integralAtRelease: number;
    readonly integralChangeDuringSaturation: number;
    readonly recommendation: string;
  }
  | { readonly status: "not_qualified"; readonly reason: string };

export type ConfiguredSaturationWindupAssessment = SaturationWindupAssessment
  | { readonly status: "unavailable"; readonly reason: string };

type EvidencePoint = {
  readonly elapsedSeconds: number;
  readonly setpoint: number;
  readonly requested: number;
  readonly applied: number;
  readonly integral: number;
};

/**
 * Assesses one actual saturation-return recording. The unlimited request is
 * deliberately retained as a separately declared runtime signal; no value is
 * reconstructed from the clamped applied output.
 */
export function assessSaturationWindup(input: SaturationWindupAssessmentInput): SaturationWindupAssessment {
  const incomplete = completeRuntimeRecording(input.recording);
  if (incomplete) return incomplete;
  if (!Number.isFinite(input.outputMinimum) || !Number.isFinite(input.outputMaximum) ||
      input.outputMinimum >= input.outputMaximum) {
    return { status: "not_qualified", reason: "Declared output limits must be finite and ordered." };
  }
  if (!input.requestedOutputId || !input.integralStateId || input.requestedOutputId === input.integralStateId) {
    return { status: "not_qualified", reason: "Distinct requested-output and retained-integral bindings are required." };
  }
  const samples = input.recording.rawSamples ?? [];
  if (samples.length < 3) {
    return { status: "not_qualified", reason: "At least three runtime samples are required for saturation-return analysis." };
  }
  const points: EvidencePoint[] = [];
  for (const sample of samples) {
    const mapped = pointFromRuntimeSample(sample, input.requestedOutputId, input.integralStateId);
    if (mapped.kind !== "point") {
      if (mapped.kind === "missing-request") {
        return { status: "not_qualified", reason: "The declared unlimited-request signal is missing from the runtime record." };
      }
      if (mapped.kind === "missing-integral") {
        return { status: "not_qualified", reason: "The declared retained-integral signal is missing from the runtime record." };
      }
      if (mapped.kind === "bad-quality") {
        return { status: "not_qualified", reason: "Saturation analysis requires good-quality requested, applied and integral samples." };
      }
      return { status: "not_qualified", reason: "Saturation analysis requires finite setpoint, requested, applied and integral values." };
    }
    points.push(mapped.point);
  }
  const chronological = points.every((point, index) => index === 0 || point.elapsedSeconds > points[index - 1].elapsedSeconds);
  if (!chronological) {
    return { status: "not_qualified", reason: "Saturation analysis requires strictly ordered runtime timestamps." };
  }
  const tolerance = Math.max(1e-9, (input.outputMaximum - input.outputMinimum) * 1e-6);
  const upperEntry = points.findIndex(point => point.requested > input.outputMaximum + tolerance &&
    Math.abs(point.applied - input.outputMaximum) <= tolerance);
  const lowerEntry = points.findIndex(point => point.requested < input.outputMinimum - tolerance &&
    Math.abs(point.applied - input.outputMinimum) <= tolerance);
  if (upperEntry < 0 && lowerEntry < 0) {
    return { status: "not_qualified", reason: "The complete runtime record contains no requested-versus-applied saturation mismatch." };
  }
  const limit = lowerEntry >= 0 && (upperEntry < 0 || lowerEntry < upperEntry) ? "lower" : "upper";
  const entryIndex = limit === "upper" ? upperEntry : lowerEntry;
  const targetReturnIndex = points.findIndex((point, index) => index > entryIndex &&
    (limit === "upper" ? point.setpoint < points[index - 1].setpoint - tolerance
      : point.setpoint > points[index - 1].setpoint + tolerance));
  if (targetReturnIndex < 0) {
    return { status: "not_qualified", reason: "The complete runtime record does not show the target changing away from the saturated demand." };
  }
  const releaseIndex = points.findIndex((point, index) => index >= targetReturnIndex &&
    (limit === "upper" ? point.applied < input.outputMaximum - tolerance : point.applied > input.outputMinimum + tolerance));
  if (releaseIndex < 0) {
    return { status: "not_qualified", reason: "The recorded return did not leave the saturated output limit." };
  }
  const entry = points[entryIndex];
  const returned = points[targetReturnIndex];
  const released = points[releaseIndex];
  return {
    status: "qualified",
    limit,
    enteredAtSeconds: entry.elapsedSeconds,
    targetReturnAtSeconds: returned.elapsedSeconds,
    releasedAtSeconds: released.elapsedSeconds,
    recoverySeconds: rounded(released.elapsedSeconds - returned.elapsedSeconds),
    peakRequestedOutput: Math.max(...points.map(point => point.requested)),
    peakAppliedOutput: Math.max(...points.map(point => point.applied)),
    maximumMismatch: Math.max(...points.map(point => Math.abs(point.requested - point.applied))),
    integralAtEntry: entry.integral,
    integralAtTargetReturn: returned.integral,
    integralAtRelease: released.integral,
    integralChangeDuringSaturation: returned.integral - entry.integral,
    recommendation: "Recorded output mismatch and retained-integral change require an anti-windup candidate before this saturation-return case can qualify.",
  };
}

/** Connects a loop's explicit adapter to a runtime record without naming conventions. */
export function assessConfiguredSaturationWindup(
  loop: Pick<ControlLoopDefinition, "controller" | "limits">,
  recording: Pick<LoopRecording, "rawSamples" | "evidence">
): ConfiguredSaturationWindupAssessment {
  const bindings = saturationWindupEvidence(loop as ControlLoopDefinition);
  if (!bindings) {
    return { status: "unavailable", reason: "Saturation-and-windup analysis needs declared unlimited-request and retained-integral bindings." };
  }
  return assessSaturationWindup({
    outputMinimum: loop.limits.outputMinimum,
    outputMaximum: loop.limits.outputMaximum,
    requestedOutputId: bindings.requestedOutputId,
    integralStateId: bindings.integralStateId,
    recording,
  });
}

function completeRuntimeRecording(recording: SaturationWindupAssessmentInput["recording"]): SaturationWindupAssessment | undefined {
  const evidence = recording.evidence;
  if (evidence.method !== "runtime_scan_buffer" || evidence.completeness !== "complete" ||
      !evidence.actualCycleTimingAvailable || evidence.missingScanRanges.length !== 0 ||
      evidence.droppedObservedSamples !== 0) {
    return { status: "not_qualified", reason: "A complete runtime-owned recording is required for saturation and windup analysis." };
  }
  return undefined;
}

function pointFromRuntimeSample(sample: WorkflowResultSample, requestedId: string, integralId: string):
  | { kind: "point"; point: EvidencePoint }
  | { kind: "missing-request" | "missing-integral" | "bad-quality" | "invalid" } {
  const requested = sample.additional?.find(channel => channel.id === requestedId);
  if (!requested) return { kind: "missing-request" };
  const integral = sample.additional?.find(channel => channel.id === integralId);
  if (!integral) return { kind: "missing-integral" };
  if (sample.quality !== "good" || requested.quality !== "good" || integral.quality !== "good") return { kind: "bad-quality" };
  if (![sample.elapsedSeconds, sample.sp, sample.appliedMv, requested.value, integral.value].every(finite)) return { kind: "invalid" };
  return { kind: "point", point: {
    elapsedSeconds: sample.elapsedSeconds,
    setpoint: sample.sp!,
    requested: requested.value!,
    applied: sample.appliedMv!,
    integral: integral.value!,
  } };
}

function finite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function rounded(value: number): number {
  return Math.round(value * 1e9) / 1e9;
}
