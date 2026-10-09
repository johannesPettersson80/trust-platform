import type { ControlLoopDefinition } from "./loopDefinition";
import type { ControlExperimentPlan } from "./experiment";
import type { LoopRecording } from "./loopMonitor";
import { evaluateRecordedRun, type RecordedRun } from "./runValidation";
import type { RuntimeVerificationResult } from "./verificationClient";
import { MainWorkflowError } from "./mainWorkflowFailure";
import { isDeepStrictEqual } from "util";

/** Identification alone accepts connected records; isolated verification keeps its stricter decoder. */
export function decodeConnectedIdentificationRecord(value: unknown): RuntimeVerificationResult | undefined {
  if (!value || typeof value !== "object") return undefined;
  const result = value as RuntimeVerificationResult;
  const acquisition = result.request?.acquisition as LoopRecording["evidence"] | undefined;
  const plan = result.request?.experiment as ControlExperimentPlan | undefined;
  if (result.request?.execution !== "connected-runtime" || !acquisition || !plan?.id
    || acquisition.method !== "runtime_scan_buffer" || acquisition.completeness !== "complete"
    || acquisition.recordingId !== result.run?.id || acquisition.programRevision !== result.projectRevision
    || result.run?.conditions.programRevision !== result.projectRevision
    || result.run?.conditions.modelRevision !== `connected-plant:${result.projectRevision}`
    || !["simulation", "physical"].includes(plan.source)
    || result.run?.evidence !== (plan.source === "simulation" ? "ST-executed" : "physical-recorded")
    || acquisition.droppedObservedSamples !== 0 || acquisition.missingScanRanges?.length !== 0
    || acquisition.observedSampleCount !== result.run?.samples.length || !acquisition.actualCycleTimingAvailable
    || acquisition.scanRange?.first !== result.run.samples[0]?.sequence
    || acquisition.scanRange?.last !== result.run.samples[result.run.samples.length - 1]?.sequence) return undefined;
  try {
    const evaluation = evaluateRecordedRun(result.run, result.evaluation.specification);
    if (!isDeepStrictEqual(evaluation, result.evaluation)) return undefined;
    return structuredClone(result);
  } catch { return undefined; }
}

/** Preserve native scan times and measured applied inputs; no replay or resampling. The record keeps the whole recording, which
 * its acquisition evidence describes; `baselineStartSeconds`, the start of the steady baseline window (B3.4), tells the fit where
 * the experiment's data begins, so the wait before it never sets the operating point, and `baselineEndSeconds` where that window
 * ends, so the operating point is taken over the window (N1.3). */
export function connectedIdentificationRecord(loop: ControlLoopDefinition, recording: LoopRecording,
  plan: ControlExperimentPlan, baselineStartSeconds = 0, baselineEndSeconds?: number): RuntimeVerificationResult {
  const acquisition = recording.evidence;
  const raw = recording.rawSamples;
  if (acquisition.method !== "runtime_scan_buffer" || acquisition.completeness !== "complete"
    || !acquisition.recordingId || !acquisition.programRevision || !raw?.length) {
    // An incomplete recording stays evidence and is never used for identification (A4.4, E4.5). Identify records a new one, so
    // the next action is Identify (A11.2); the runtime's own reason is kept, and the message says what the next attempt changes (A11.1).
    throw new MainWorkflowError(`Identification requires the complete recording from the connected runtime. ${acquisition.reason} `
      + "The incomplete recording is kept as evidence, and Identify plant records a new one.",
    { cause: "model-or-data", nextAction: "identify" });
  }
  const bindings = [loop.signals.processVariable, loop.signals.manipulatedVariable,
    ...(loop.signals.setpoint ? [loop.signals.setpoint] : []), ...(loop.signals.additional ?? [])];
  const samples: RecordedRun["samples"] = raw.map(sample => ({ timeSeconds: sample.elapsedSeconds, sequence: sample.scan,
    values: Object.fromEntries(bindings.map(binding => {
      const additional = "id" in binding ? sample.additional?.find(value => value.id === binding.id) : undefined;
      const value = binding === loop.signals.processVariable ? sample.pv : binding === loop.signals.manipulatedVariable ? sample.appliedMv
        : binding === loop.signals.setpoint ? sample.sp : additional?.value ?? null;
      return [binding.symbol, { value, quality: additional?.quality ?? sample.quality }];
    })) }));
  const durationSeconds = samples[samples.length - 1].timeSeconds;
  const run: RecordedRun = { id: acquisition.recordingId, evidence: plan.source === "simulation" ? "ST-executed" : "physical-recorded",
    parameters: {}, channels: bindings.map(binding => ({ id: binding.symbol, unit: binding.unit })), samples,
    conditions: { initialState: Object.fromEntries(Object.entries(samples[0].values).filter(([, value]) => value.value !== null).map(([id, value]) => [id, value.value!])),
      events: [], durationSeconds, sampleTimesSeconds: samples.map(sample => sample.timeSeconds), seed: plan.seed ?? 1,
      modelRevision: `connected-plant:${acquisition.programRevision}`, programRevision: acquisition.programRevision,
      parameterSchemaRevision: loop.revision } };
  const evaluation = evaluateRecordedRun(run, bindings.filter(binding => binding !== loop.signals.setpoint).map(binding => ({
    id: `bounds:${binding.symbol}`, kind: "bounds", channel: binding.symbol, window: { start: 0, end: durationSeconds },
    minimum: binding === loop.signals.manipulatedVariable ? loop.limits.outputMinimum : binding.range.minimum,
    maximum: binding === loop.signals.manipulatedVariable ? loop.limits.outputMaximum : binding.range.maximum,
  })));
  return { projectRevision: acquisition.programRevision, request: { execution: "connected-runtime", loop_revision: loop.revision,
    acquisition, experiment: plan, ...(baselineStartSeconds > 0 ? { baselineStartSeconds } : {}),
    ...(baselineEndSeconds !== undefined && baselineEndSeconds > baselineStartSeconds ? { baselineEndSeconds } : {}) }, run, evaluation };
}
