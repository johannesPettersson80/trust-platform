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
import { simulationTarget } from "./simulationTarget";
import { CONNECTION_LOSS_TIMEOUT_MS } from "./controlStudioLimits";

export type ControlEvidenceSource = "Simulation" | "Physical";
export type ControlSignalQuality = "good" | "forced" | "uncertain" | "stale" | "bad";

export interface ControlMonitorSignal {
  symbol: string;
  /** Absent for a program variable that has no address. */
  address?: string;
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
  return Math.max(CONNECTION_LOSS_TIMEOUT_MS, loop.observedSampleTiming.nominalPeriodMs * 5);
}

/** Projects one synchronized runtime scan; it never owns or polls a transport. */
export class ControlLoopMonitor {
  private readonly staleAfterMs: number;
  private lastScan: number | undefined;
  private lastNewScanAtMs: number | undefined;
  private runtimeRecording = false;

  setRuntimeRecording(active: boolean): void { this.runtimeRecording = active; }
  /** A confirmed runtime replacement resets freshness, preserving retained data. */
  resetRuntimeObservation(): void {
    this.lastScan = undefined;
    this.lastNewScanAtMs = undefined;
  }

  constructor(options: MonitorOptions) {
    this.staleAfterMs = options.staleAfterMs;
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

  /** Projects one coherent scan. observedAtMs is the host time the scan was received; the scan is stale once it has not
   * advanced for staleAfterMs of that clock (A3.4). */
  observeReadings(loop: ControlLoopDefinition, source: ControlEvidenceSource,
    readings: CoherentLoopReadings, observedAtMs: number, staleAfterMs = this.staleAfterMs): LoopMonitorSample {
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
      observedAtMs - this.lastNewScanAtMs > staleAfterMs
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
      recording: this.runtimeRecording,
    };
    return sample;
  }
}

function readSignal(binding: ControlSignalBinding, state: IoState): ControlMonitorSignal | undefined {
  // The process image has no entry for a program variable without an address; the symbolic observation reads it.
  if (binding.address === undefined) return undefined;
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
  return simulationTarget(lifecycle.status) ? "Simulation" : "Physical";
}

function saturation(value: number, loop: ControlLoopDefinition): AvailableLoopSample["saturation"] {
  if (value <= loop.limits.outputMinimum) return "minimum";
  if (value >= loop.limits.outputMaximum) return "maximum";
  return "none";
}

function missingReason(label: string, binding: ControlSignalBinding, scan: number): string {
  return `${label} ${binding.address ?? binding.symbol} is missing from runtime scan ${scan}`;
}

function unavailable(source: ControlEvidenceSource, loopId: string, reason: string): UnavailableLoopSample {
  return { availability: "unavailable", source, loopId, reason };
}
