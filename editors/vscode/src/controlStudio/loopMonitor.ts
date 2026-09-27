import { controllerModeAddress } from "./controllerModeBinding";
import type { WorkflowResultSample, WorkflowRecordingProvenance } from "./workflowResult";
import type { IoEntry, IoState } from "../io-panel/types";
import type { RuntimeLifecycleSnapshot } from "../runtimeLifecycle";
import type {
  ControlAuxiliarySignalBinding,
  ControlAuxiliarySignalRole,
  ControlLoopDefinition,
  ControlSignalBinding,
} from "./loopDefinition";

export type ControlEvidenceSource = "Simulation" | "Physical";
export type ControlSignalQuality = "good" | "forced" | "uncertain" | "stale" | "bad";

export interface ControlMonitorSignal {
  symbol: string;
  address: string;
  value: number;
  unit: string;
  quality: ControlSignalQuality;
}

export interface ControlMonitorAuxiliarySignal extends ControlMonitorSignal {
  id: string;
  channel: string;
  role: ControlAuxiliarySignalRole;
  /** Human-readable label derived from the declared signal id; the ST symbol stays available alongside it. */
  displayName: string;
}

export interface AvailableLoopSample {
  availability: "available";
  /** Confirmed debug-session identity, independent of a reused runtime endpoint. */
  runtimeSessionId?: string;
  source: ControlEvidenceSource;
  loopId: string;
  loopRevision: string;
  controller: {
    instance: string;
    revision: string;
    mode: "Auto" | "Manual" | "Unavailable";
  };
  scan: number;
  observedAtMs: number;
  setpoint?: ControlMonitorSignal;
  processVariable: ControlMonitorSignal;
  manipulatedVariable: ControlMonitorSignal;
  additional?: readonly ControlMonitorAuxiliarySignal[];
  outputLimits: { minimum: number; maximum: number };
  rateLimits: { upPerSecond?: number; downPerSecond?: number };
  saturation: "minimum" | "maximum" | "none";
  recording: boolean;
}

export interface UnavailableLoopSample {
  availability: "unavailable";
  source: ControlEvidenceSource;
  loopId: string;
  reason: string;
}

export type LoopMonitorSample = AvailableLoopSample | UnavailableLoopSample;

export interface MissingScanRange {
  first: number;
  last: number;
}

export interface LoopRecordingEvidence {
  readonly recordingId?: string;
  programRevision?: string;
  method: "ui_observed_snapshots" | "runtime_scan_buffer";
  completeness: "incomplete" | "complete";
  observedSampleCount: number;
  scanRange?: { first: number; last: number };
  missingScanRanges: readonly MissingScanRange[];
  droppedObservedSamples: number;
  actualCycleTimingAvailable: boolean;
  freshnessAvailable: boolean;
  reason: string;
}

export interface LoopRecording {
  rawSamples?: readonly WorkflowResultSample[];
  samples: readonly AvailableLoopSample[];
  evidence: LoopRecordingEvidence;
  provenance?: WorkflowRecordingProvenance;
}

export interface LoopHealthSummary {
  sampleCount: number;
  meanAbsoluteError: number | null;
  saturationSamples: number;
  timing: string;
}

/** All fields must originate in one runtime scan; transports decode provenance before projection. */
export interface CoherentLoopReadings {
  running: boolean;
  scan: number | undefined;
  controllerMode: AvailableLoopSample["controller"]["mode"];
  setpoint?: ControlMonitorSignal;
  processVariable?: ControlMonitorSignal;
  manipulatedVariable?: ControlMonitorSignal;
  additional: readonly (ControlMonitorAuxiliarySignal | undefined)[];
}

interface MonitorOptions {
  staleAfterMs: number;
  recordingCapacity: number;
}

/** Keeps slow declared loops current without weakening the minimum live-loop guard. */
export function controlLoopStaleAfterMs(
  loop: Pick<ControlLoopDefinition, "observedSampleTiming">
): number {
  return Math.max(1_500, loop.observedSampleTiming.nominalPeriodMs * 5);
}

/** Projects one synchronized runtime scan; it never owns or polls a transport. */
export class ControlLoopMonitor {
  private readonly staleAfterMs: number;
  private readonly recordingCapacity: number;
  private lastScan: number | undefined;
  private lastNewScanAtMs: number | undefined;
  private recording = false;
  private runtimeRecording = false;

  setRuntimeRecording(active: boolean): void { this.runtimeRecording = active; }
  /** A confirmed runtime replacement resets freshness, preserving retained data. */
  resetRuntimeObservation(): void {
    this.lastScan = undefined;
    this.lastNewScanAtMs = undefined;
  }
  private samples: AvailableLoopSample[] = [];
  private lastRecordedScan: number | undefined;
  private droppedObservedSamples = 0;

  constructor(options: MonitorOptions) {
    this.staleAfterMs = options.staleAfterMs;
    this.recordingCapacity = Math.max(1, Math.floor(options.recordingCapacity));
  }

  startRecording(): void {
    this.samples = [];
    this.lastRecordedScan = undefined;
    this.droppedObservedSamples = 0;
    this.recording = true;
  }

  stopRecording(): LoopRecording {
    this.recording = false;
    const samples = this.samples.map(copySample);
    const firstRecordedScan = samples[0]?.scan;
    const lastRecordedScan = samples[samples.length - 1]?.scan;
    const missingScanRanges = missingScanRangesFor(samples);
    return {
      samples,
      evidence: {
        method: "ui_observed_snapshots",
        completeness: "incomplete",
        observedSampleCount: samples.length,
        ...(firstRecordedScan === undefined || lastRecordedScan === undefined
          ? {} : { scanRange: { first: firstRecordedScan, last: lastRecordedScan } }),
        missingScanRanges,
        droppedObservedSamples: this.droppedObservedSamples,
        actualCycleTimingAvailable: false,
        freshnessAvailable: false,
        reason: incompleteRecordingReason(
          samples.length,
          firstRecordedScan,
          lastRecordedScan,
          missingScanRanges,
          this.droppedObservedSamples
        ),
      },
    };
  }

  observe(
    loop: ControlLoopDefinition,
    lifecycle: RuntimeLifecycleSnapshot,
    observedAtMs: number
  ): LoopMonitorSample {
    return this.observeReadings(loop, evidenceSource(lifecycle), {
      running: lifecycle.status.running, scan: lifecycle.ioState.scan,
      controllerMode: readMode(controllerModeAddress(loop), loop.controller.modeWhenTrue ?? "Auto", lifecycle.ioState),
      setpoint: loop.signals.setpoint ? readSignal(loop.signals.setpoint, lifecycle.ioState) : undefined,
      processVariable: readSignal(loop.signals.processVariable, lifecycle.ioState),
      manipulatedVariable: readSignal(loop.signals.manipulatedVariable, lifecycle.ioState),
      additional: (loop.signals.additional ?? []).map(binding => readAuxiliarySignal(binding, lifecycle.ioState)),
    }, observedAtMs);
  }

  observeReadings(loop: ControlLoopDefinition, source: ControlEvidenceSource,
    readings: CoherentLoopReadings, observedAtMs: number): LoopMonitorSample {
    const scan = readings.scan;
    if (!readings.running || scan === undefined) {
      if (!readings.running) {
        this.resetRuntimeObservation();
      }
      return unavailable(source, loop.id, "Runtime is not producing scans");
    }
    if (this.lastScan !== undefined && scan < this.lastScan) {
      return unavailable(source, loop.id, `Runtime scan moved backwards from ${this.lastScan} to ${scan}`);
    }
    if (scan !== this.lastScan) {
      this.lastScan = scan;
      this.lastNewScanAtMs = observedAtMs;
    } else if (
      this.lastNewScanAtMs !== undefined &&
      observedAtMs - this.lastNewScanAtMs > this.staleAfterMs
    ) {
      return unavailable(source, loop.id, "Runtime scan is stale");
    }

    const setpoint = loop.signals.setpoint ? readings.setpoint : undefined;
    if (loop.signals.setpoint && !setpoint) {
      return unavailable(source, loop.id, missingReason("Setpoint", loop.signals.setpoint, scan));
    }
    const processVariable = readings.processVariable;
    if (!processVariable) {
      return unavailable(source, loop.id, missingReason("Process variable", loop.signals.processVariable, scan));
    }
    const manipulatedVariable = readings.manipulatedVariable;
    if (!manipulatedVariable) {
      return unavailable(source, loop.id, missingReason("Manipulated variable", loop.signals.manipulatedVariable, scan));
    }
    const additional: ControlMonitorAuxiliarySignal[] = [];
    for (const [index, binding] of (loop.signals.additional ?? []).entries()) {
      const signal = readings.additional[index];
      if (!signal) return unavailable(source, loop.id, missingReason(binding.symbol, binding, scan));
      additional.push(signal);
    }

    const sample: AvailableLoopSample = {
      availability: "available",
      source,
      loopId: loop.id,
      loopRevision: loop.revision,
      controller: {
        instance: loop.controller.instance,
        revision: loop.controller.revision,
        mode: readings.controllerMode,
      },
      scan,
      observedAtMs,
      ...(setpoint ? { setpoint } : {}),
      processVariable,
      manipulatedVariable,
      ...(additional.length === 0 ? {} : { additional }),
      outputLimits: {
        minimum: loop.limits.outputMinimum,
        maximum: loop.limits.outputMaximum,
      },
      rateLimits: {
        ...(loop.limits.rateUpPerS === undefined ? {} : { upPerSecond: loop.limits.rateUpPerS }),
        ...(loop.limits.rateDownPerS === undefined ? {} : { downPerSecond: loop.limits.rateDownPerS }),
      },
      saturation: saturation(manipulatedVariable.value, loop),
      recording: this.recording || this.runtimeRecording,
    };
    if (this.recording && scan !== this.lastRecordedScan) {
      this.samples.push(copySample(sample));
      this.lastRecordedScan = scan;
      if (this.samples.length > this.recordingCapacity) {
        this.samples.shift();
        this.droppedObservedSamples += 1;
      }
    }
    return sample;
  }
}

function incompleteRecordingReason(
  observed: number,
  first: number | undefined,
  last: number | undefined,
  gaps: readonly MissingScanRange[],
  dropped: number
): string {
  if (first !== undefined && last !== undefined && gaps.length > 0) {
    const expected = last - first + 1;
    const ranges = gaps.map((range) => range.first === range.last
      ? String(range.first) : `${range.first}\u2013${range.last}`);
    const listed = ranges.length === 1
      ? ranges[0] : `${ranges.slice(0, -1).join(", ")} and ${ranges[ranges.length - 1]}`;
    const missing = `Incomplete recording \u2014 UI observed ${observed} of ${expected} scans; scans ${listed} were not acquired`;
    return dropped > 0
      ? `${missing}; ${dropped} UI-observed ${dropped === 1 ? "sample was" : "samples were"} dropped by the bounded editor buffer.`
      : `${missing}.`;
  }
  if (dropped > 0) {
    return `Incomplete recording \u2014 ${dropped} UI-observed ${dropped === 1 ? "sample was" : "samples were"} dropped by the bounded editor buffer.`;
  }
  return "Incomplete recording \u2014 UI-observed snapshots do not prove every runtime scan was acquired.";
}

function missingScanRangesFor(samples: readonly AvailableLoopSample[]): MissingScanRange[] {
  const ranges: MissingScanRange[] = [];
  for (let index = 1; index < samples.length; index += 1) {
    const previous = samples[index - 1].scan;
    const current = samples[index].scan;
    if (current > previous + 1) ranges.push({ first: previous + 1, last: current - 1 });
  }
  return ranges;
}

export function summarizeLoopHealth(samples: readonly AvailableLoopSample[]): LoopHealthSummary {
  const errorSamples = samples.filter((sample) => sample.setpoint !== undefined);
  const meanAbsoluteError = samples.length > 0 && errorSamples.length === samples.length
    ? errorSamples.reduce((sum, sample) =>
      sum + Math.abs(sample.setpoint!.value - sample.processVariable.value), 0) / errorSamples.length
    : null;
  const timing = samples.length < 3
    ? `${numberWord(samples.length)} ${samples.length === 1 ? "sample is" : "samples are"} insufficient for oscillation or noise diagnostics.`
    : "Timing is available for descriptive analysis.";
  return {
    sampleCount: samples.length,
    meanAbsoluteError,
    saturationSamples: samples.filter((sample) => sample.saturation !== "none").length,
    timing,
  };
}

function readSignal(binding: ControlSignalBinding, state: IoState): ControlMonitorSignal | undefined {
  const entry = entries(state).find((candidate) => candidate.address === binding.address);
  if (!entry) return undefined;
  const value = numericIoValue(entry.value);
  if (!Number.isFinite(value)) return undefined;
  return {
    symbol: binding.symbol,
    address: binding.address,
    value,
    unit: binding.unit,
    quality: entry.forced ? "forced" : (entry.quality ?? "good"),
  };
}

function readAuxiliarySignal(
  binding: ControlAuxiliarySignalBinding,
  state: IoState
): ControlMonitorAuxiliarySignal | undefined {
  const signal = readSignal(binding, state);
  return signal
    ? { id: binding.id, channel: binding.channel, role: binding.role, displayName: humanizeSignalIdentifier(binding.id), ...signal }
    : undefined;
}

/**
 * Turns a kebab- or snake-case lesson signal id (e.g. "door-open-indication") into a
 * readable label ("Door open indication") so Monitor -> Signals does not force the
 * learner to translate the loop's ST symbols back into the lesson's own vocabulary.
 */
export function humanizeSignalIdentifier(id: string): string {
  const words = id.split(/[-_]+/).filter((word) => word.length > 0);
  if (words.length === 0) return id;
  return words.map((word, index) => (index === 0 ? capitalizeWord(word) : word.toLowerCase())).join(" ");
}

function capitalizeWord(word: string): string {
  return word.length === 0 ? word : `${word[0].toUpperCase()}${word.slice(1).toLowerCase()}`;
}

function numericIoValue(value: string): number {
  if (value.trim().toUpperCase() === "TRUE") return 1;
  if (value.trim().toUpperCase() === "FALSE") return 0;
  return Number(value);
}

function readMode(
  address: string | undefined,
  modeWhenTrue: "Auto" | "Manual",
  state: IoState
): "Auto" | "Manual" | "Unavailable" {
  if (!address) return "Unavailable";
  const entry = entries(state).find((candidate) => candidate.address === address);
  if (!entry) return "Unavailable";
  const value = entry.value.trim().toUpperCase();
  if (value === "TRUE" || value === "1") return modeWhenTrue;
  if (value === "FALSE" || value === "0") return modeWhenTrue === "Auto" ? "Manual" : "Auto";
  return "Unavailable";
}

function entries(state: IoState): readonly IoEntry[] {
  return [...state.inputs, ...state.outputs, ...state.memory];
}

function evidenceSource(lifecycle: RuntimeLifecycleSnapshot): ControlEvidenceSource {
  return lifecycle.status.runtimeMode === "simulate" ? "Simulation" : "Physical";
}

function saturation(value: number, loop: ControlLoopDefinition): AvailableLoopSample["saturation"] {
  if (value <= loop.limits.outputMinimum) return "minimum";
  if (value >= loop.limits.outputMaximum) return "maximum";
  return "none";
}

function missingReason(label: string, binding: ControlSignalBinding, scan: number): string {
  return `${label} ${binding.address} is missing from runtime scan ${scan}`;
}

function unavailable(source: ControlEvidenceSource, loopId: string, reason: string): UnavailableLoopSample {
  return { availability: "unavailable", source, loopId, reason };
}

function copySample(sample: AvailableLoopSample): AvailableLoopSample {
  return {
    ...sample,
    controller: { ...sample.controller },
    ...(sample.setpoint ? { setpoint: { ...sample.setpoint } } : {}),
    processVariable: { ...sample.processVariable },
    manipulatedVariable: { ...sample.manipulatedVariable },
    ...(sample.additional ? { additional: sample.additional.map((signal) => ({ ...signal })) } : {}),
    outputLimits: { ...sample.outputLimits },
    rateLimits: { ...sample.rateLimits },
  };
}

function numberWord(value: number): string {
  return ["Zero", "One", "Two"][value] ?? String(value);
}
