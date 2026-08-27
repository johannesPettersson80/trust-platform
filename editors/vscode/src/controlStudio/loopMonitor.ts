import type { IoEntry, IoState } from "../io-panel/types";
import type { RuntimeLifecycleSnapshot } from "../runtimeLifecycle";
import type { ControlLoopDefinition, ControlSignalBinding } from "./loopDefinition";

export type ControlEvidenceSource = "Simulation" | "Physical";
export type ControlSignalQuality = "good" | "forced";

export interface ControlMonitorSignal {
  symbol: string;
  address: string;
  value: number;
  unit: string;
  quality: ControlSignalQuality;
}

export interface AvailableLoopSample {
  availability: "available";
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
  setpoint: ControlMonitorSignal;
  processVariable: ControlMonitorSignal;
  manipulatedVariable: ControlMonitorSignal;
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

export interface LoopHealthSummary {
  sampleCount: number;
  meanAbsoluteError: number;
  saturationSamples: number;
  timing: string;
}

interface MonitorOptions {
  staleAfterMs: number;
  recordingCapacity: number;
}

/** Projects one synchronized runtime scan; it never owns or polls a transport. */
export class ControlLoopMonitor {
  private readonly staleAfterMs: number;
  private readonly recordingCapacity: number;
  private lastScan: number | undefined;
  private lastNewScanAtMs: number | undefined;
  private recording = false;
  private samples: AvailableLoopSample[] = [];
  private lastRecordedScan: number | undefined;

  constructor(options: MonitorOptions) {
    this.staleAfterMs = options.staleAfterMs;
    this.recordingCapacity = Math.max(1, Math.floor(options.recordingCapacity));
  }

  startRecording(): void {
    this.samples = [];
    this.lastRecordedScan = undefined;
    this.recording = true;
  }

  stopRecording(): AvailableLoopSample[] {
    this.recording = false;
    return this.samples.map(copySample);
  }

  observe(
    loop: ControlLoopDefinition,
    lifecycle: RuntimeLifecycleSnapshot,
    observedAtMs: number
  ): LoopMonitorSample {
    const source = evidenceSource(lifecycle);
    const scan = lifecycle.ioState.scan;
    if (!lifecycle.status.running || scan === undefined) {
      return unavailable(source, loop.id, "Runtime is not producing scans");
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

    const setpoint = readSignal(loop.signals.setpoint, lifecycle.ioState);
    if (!setpoint) return unavailable(source, loop.id, missingReason("Setpoint", loop.signals.setpoint, scan));
    const processVariable = readSignal(loop.signals.processVariable, lifecycle.ioState);
    if (!processVariable) {
      return unavailable(source, loop.id, missingReason("Process variable", loop.signals.processVariable, scan));
    }
    const manipulatedVariable = readSignal(loop.signals.manipulatedVariable, lifecycle.ioState);
    if (!manipulatedVariable) {
      return unavailable(source, loop.id, missingReason("Manipulated variable", loop.signals.manipulatedVariable, scan));
    }

    const sample: AvailableLoopSample = {
      availability: "available",
      source,
      loopId: loop.id,
      loopRevision: loop.revision,
      controller: {
        instance: loop.controller.instance,
        revision: loop.controller.revision,
        mode: readMode(loop.controller.modeSignal, lifecycle.ioState),
      },
      scan,
      observedAtMs,
      setpoint,
      processVariable,
      manipulatedVariable,
      outputLimits: {
        minimum: loop.limits.outputMinimum,
        maximum: loop.limits.outputMaximum,
      },
      rateLimits: {
        ...(loop.limits.rateUpPerS === undefined ? {} : { upPerSecond: loop.limits.rateUpPerS }),
        ...(loop.limits.rateDownPerS === undefined ? {} : { downPerSecond: loop.limits.rateDownPerS }),
      },
      saturation: saturation(manipulatedVariable.value, loop),
      recording: this.recording,
    };
    if (this.recording && scan !== this.lastRecordedScan) {
      this.samples.push(copySample(sample));
      this.lastRecordedScan = scan;
      if (this.samples.length > this.recordingCapacity) this.samples.shift();
    }
    return sample;
  }
}

export function summarizeLoopHealth(samples: readonly AvailableLoopSample[]): LoopHealthSummary {
  const meanAbsoluteError = samples.length === 0
    ? 0
    : samples.reduce((sum, sample) =>
      sum + Math.abs(sample.setpoint.value - sample.processVariable.value), 0) / samples.length;
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
  const value = Number(entry.value);
  if (!Number.isFinite(value)) return undefined;
  return {
    symbol: binding.symbol,
    address: binding.address,
    value,
    unit: binding.unit,
    quality: entry.forced ? "forced" : "good",
  };
}

function readMode(address: string | undefined, state: IoState): "Auto" | "Manual" | "Unavailable" {
  if (!address) return "Unavailable";
  const entry = entries(state).find((candidate) => candidate.address === address);
  if (!entry) return "Unavailable";
  const value = entry.value.trim().toUpperCase();
  if (value === "TRUE" || value === "1") return "Auto";
  if (value === "FALSE" || value === "0") return "Manual";
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
    setpoint: { ...sample.setpoint },
    processVariable: { ...sample.processVariable },
    manipulatedVariable: { ...sample.manipulatedVariable },
    outputLimits: { ...sample.outputLimits },
    rateLimits: { ...sample.rateLimits },
  };
}

function numberWord(value: number): string {
  return ["Zero", "One", "Two"][value] ?? String(value);
}
