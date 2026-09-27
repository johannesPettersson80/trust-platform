import { createHash } from "crypto";
import { decodePidLaw } from "./controllerLaw";
import type { AntiWindupComparisonRun, SaturationReturnConditions } from "./antiWindupComparison";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopRecording } from "./loopMonitor";
import type { RecordedRun } from "./runValidation";
import { assessConfiguredSaturationWindup } from "./saturationWindup";
import type { WorkflowResultSample } from "./workflowResult";

/** Converts an isolated actual-ST run into the only input accepted by WF-008 comparison. */
export function antiWindupRunFromRecordedSt(loop: ControlLoopDefinition, run: RecordedRun): AntiWindupComparisonRun {
  if (run.evidence !== "ST-executed") throw new Error("Anti-windup comparison requires actual ST execution evidence.");
  const law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  if (!law?.antiWindup) throw new Error("A typed back-calculation anti-windup adapter is required.");
  const trackingGain = run.parameters[law.antiWindup.trackingGainSymbol];
  if (!Number.isFinite(trackingGain)) throw new Error("The actual ST run did not retain its declared tracking gain.");
  const requested = loop.signals.additional?.find(signal => signal.id === loop.controller.saturationEvidence?.requestedOutputId);
  const integral = loop.signals.additional?.find(signal => signal.id === loop.controller.saturationEvidence?.integralStateId);
  if (!requested || !integral) throw new Error("Declared requested-output and retained-integral bindings are required.");
  const rawSamples = run.samples.map(sample => sampleFromRun(loop, sample, requested.symbol, requested.id, integral.symbol, integral.id));
  const recording: Pick<LoopRecording, "rawSamples" | "evidence"> = {
    rawSamples,
    evidence: {
      method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: rawSamples.length,
      missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true, freshnessAvailable: true,
      programRevision: run.conditions.programRevision, reason: "Complete isolated actual-ST saturation-return run.",
    },
  };
  const assessment = assessConfiguredSaturationWindup(loop, recording);
  if (assessment.status !== "qualified") {
    throw new Error(assessment.status === "unavailable" ? assessment.reason : `Saturation-return evidence is not qualified: ${assessment.reason}`);
  }
  return {
    trackingGain,
    conditions: comparisonConditions(loop, run),
    assessment,
    oscillationDetected: oscillationAfterTargetReturn(rawSamples, assessment.targetReturnAtSeconds),
  };
}

function sampleFromRun(
  loop: ControlLoopDefinition,
  sample: RecordedRun["samples"][number],
  requestedSymbol: string,
  requestedId: string,
  integralSymbol: string,
  integralId: string,
): WorkflowResultSample {
  const setpoint = required(sample, loop.signals.setpoint?.symbol, "setpoint");
  const processValue = required(sample, loop.signals.processVariable.symbol, "process value");
  const applied = required(sample, loop.signals.manipulatedVariable.symbol, "applied output");
  const requested = required(sample, requestedSymbol, "requested-output");
  const integral = required(sample, integralSymbol, "retained-integral");
  const quality = [setpoint, processValue, applied, requested, integral].every(value => value.quality === "good") ? "good" : "bad";
  return {
    elapsedSeconds: sample.timeSeconds,
    observedAtMs: Math.round(sample.timeSeconds * 1000),
    actualCycleSeconds: null,
    scan: sample.sequence,
    source: "Simulation",
    loopRevision: loop.revision,
    sp: setpoint.value,
    pv: processValue.value,
    requestedMv: requested.value,
    appliedMv: applied.value,
    units: { sp: loop.signals.setpoint?.unit ?? "process unit", pv: loop.signals.processVariable.unit, mv: loop.signals.manipulatedVariable.unit },
    quality,
    freshnessMs: 0,
    controllerMode: "Auto",
    outputLimits: { minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum },
    rateLimits: {},
    safetyState: "Normal",
    events: [],
    additional: [
      { id: requestedId, channel: requestedId, unit: loop.signals.manipulatedVariable.unit, role: "state", value: requested.value, quality: quality === "good" ? "good" : "bad" },
      { id: integralId, channel: integralId, unit: loop.signals.manipulatedVariable.unit, role: "state", value: integral.value, quality: quality === "good" ? "good" : "bad" },
    ],
  };
}

function required(sample: RecordedRun["samples"][number], symbol: string | undefined, name: string): { value: number; quality: "good" | "bad" | "forced" | "missing" } {
  const observed = symbol ? sample.values[symbol] : undefined;
  if (!observed || !Number.isFinite(observed.value)) throw new Error(`The declared ${name} observation is missing from the isolated ST run.`);
  return observed as { value: number; quality: "good" | "bad" | "forced" | "missing" };
}

function comparisonConditions(loop: ControlLoopDefinition, run: RecordedRun): SaturationReturnConditions {
  const conditions = run.conditions as RecordedRun["conditions"] & { simulationConfigRevision?: unknown };
  if (conditions.sampleTimesSeconds.length < 2 || !Number.isFinite(conditions.sampleTimesSeconds[0]) ||
      !Number.isFinite(conditions.sampleTimesSeconds[1]) || conditions.sampleTimesSeconds[1] <= conditions.sampleTimesSeconds[0]) {
    throw new Error("The isolated ST run has no valid recorded sample interval.");
  }
  return {
    loopId: loop.id,
    loopRevision: loop.revision,
    source: "simulation",
    scenarioDigest: digest(conditions),
    plantInitialConditionDigest: digest({ modelRevision: conditions.modelRevision, simulationConfigRevision: conditions.simulationConfigRevision,
      initialState: conditions.initialState }),
    controllerInitialConditionDigest: digest(conditions.initialState),
    samplePeriodMs: Math.round((conditions.sampleTimesSeconds[1] - conditions.sampleTimesSeconds[0]) * 1000),
    durationMs: Math.round(conditions.durationSeconds * 1000),
    outputMinimum: loop.limits.outputMinimum,
    outputMaximum: loop.limits.outputMaximum,
  };
}

function oscillationAfterTargetReturn(samples: readonly WorkflowResultSample[], targetReturnAtSeconds: number): boolean {
  let sign = 0;
  let changes = 0;
  for (const sample of samples) {
    if (sample.elapsedSeconds < targetReturnAtSeconds || sample.sp === null || sample.pv === null) continue;
    const error = sample.sp - sample.pv;
    const next = Math.abs(error) <= 1e-9 ? 0 : error > 0 ? 1 : -1;
    if (next !== 0 && sign !== 0 && next !== sign) changes++;
    if (next !== 0) sign = next;
  }
  return changes >= 2;
}

function digest(value: unknown): string {
  return `sha256:${createHash("sha256").update(canonical(value), "utf8").digest("hex")}`;
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value as Record<string, unknown>).sort().map(key => `${JSON.stringify(key)}:${canonical((value as Record<string, unknown>)[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}
