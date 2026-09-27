import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMonitorSample, LoopRecording, LoopRecordingEvidence } from "./loopMonitor";
import type { RecordingRecoveryOwner } from "./runtimeRecording";

export type ControlExperimentKind = "observation" | "setpoint_step" | "load_step" |
  "mv_step" | "mv_pulse" | "relay" | "prbs" | "multisine" | "output_step" |
  "project_sequence";

/** The automatic request is resolved to a concrete, reviewable runtime plan. */
export type ControlExperimentRequestKind = ControlExperimentKind | "automatic";

export interface ControlExperimentChoice {
  value: Exclude<ControlExperimentRequestKind, "output_step" | "project_sequence">;
  label: string;
}

export interface ControlExperimentDefaults {
  execution: "control-studio" | "project-scripted";
  kind: ControlExperimentRequestKind;
  amplitude: number;
  durationSeconds: number;
  seed: number;
}

export function controlExperimentDefaults(loop: ControlLoopDefinition): ControlExperimentDefaults {
  const configured = loop.experiment;
  if (configured) {
    if (configured.execution !== "project-scripted" && configured.kind === "observation" && configured.amplitude === 0) {
      return { ...configured, kind: "automatic", amplitude: automaticDefaultAmplitude(loop),
        durationSeconds: Math.max(configured.durationSeconds, 10) };
    }
    return configured;
  }
  return {
    execution: "control-studio",
    kind: "automatic",
    amplitude: automaticDefaultAmplitude(loop),
    durationSeconds: 30,
    seed: 1,
  };
}

/** User-facing experiments; output_step remains accepted only as a legacy wire alias. */
export function controlExperimentChoices(): readonly ControlExperimentChoice[] {
  return [
    { value: "automatic", label: "Automatic bounded excitation" },
    { value: "observation", label: "Observe without excitation" },
    { value: "setpoint_step", label: "Setpoint step" },
    { value: "load_step", label: "Load disturbance step" },
    { value: "mv_step", label: "Manual output step" },
    { value: "mv_pulse", label: "Manual output pulse" },
    { value: "relay", label: "Relay feedback" },
    { value: "prbs", label: "PRBS" },
    { value: "multisine", label: "Multisine" },
  ];
}

export interface ControlExperimentPoint { atMs: number; value: number }

export interface ControlExperimentRecoveryScope {
  projectId: string;
  loopId: string;
  loopRevision: string;
}

export interface ControlExperimentRecovery {
  schemaVersion: 1 | 2;
  experimentId: string;
  projectId: string;
  loopId: string;
  loopRevision: string;
  plan?: ControlExperimentPlan;
  recordingOwner?: RecordingRecoveryOwner;
}

export interface ControlRelayFeedbackPlan {
  processVariable: string;
  address: string;
  valueType: string;
  unit: string;
  center: number;
  hysteresis: number;
  lowerThreshold: number;
  upperThreshold: number;
  lowOutput: number;
  highOutput: number;
}

export interface ControlExperimentPlan {
  id: string;
  source: "simulation" | "physical";
  kind: ControlExperimentKind;
  affectedSignal: string;
  address: string;
  valueType: string;
  from: number | boolean;
  to: number | boolean;
  unit: string;
  minimum: number;
  maximum: number;
  durationMs: number;
  expectedScan: number;
  stopOnStaleMs: number;
  stopConditions: string[];
  restorationValue: number | boolean;
  requiresPhysicalApproval: boolean;
  trajectory: ControlExperimentPoint[];
  abortBinding?: { symbol: string; address: string; valueType: "BOOL"; assertedValue: boolean };
  relayFeedback?: ControlRelayFeedbackPlan;
  intervalMs?: number;
  seed?: number;
  frequenciesHz?: number[];
}

/** A terminal Test record assigned to the independent Identify workflow. */
export interface ExperimentIdentificationRecord {
  readonly experimentId: string;
  readonly role: "estimation" | "validation";
  readonly outcome: "completed" | "failed";
  readonly qualification: "qualified" | "not_qualified";
  readonly plan: Readonly<ControlExperimentPlan>;
  readonly terminal: Readonly<ExperimentIdentificationTerminal>;
  readonly samples: readonly ExperimentIdentificationSample[];
  readonly acquisition?: Readonly<LoopRecordingEvidence>;
}

export interface ExperimentIdentificationTerminal {
  readonly experiment_id: string;
  readonly status: "completed" | "aborted" | "failed" | "failed_to_restore";
  readonly stop_reason?: string | null;
  readonly restoration: "restored" | "not_required" | "failed";
  readonly restoration_reason?: string | null;
}

export interface ExperimentIdentificationSample {
  readonly timeSeconds: number;
  readonly input: number;
  readonly output: number;
  readonly quality: "good" | "uncertain" | "bad";
  readonly inputUnit?: string;
  readonly outputUnit?: string;
  readonly clipped?: boolean;
  readonly saturated?: boolean;
}

/**
 * Keeps Test outcomes as immutable inputs to Identify. This deliberately owns
 * no fitting: IdentificationWorkflow remains the sole model-fitting state machine.
 */
export class ExperimentIdentificationLedger {
  private entries: ExperimentIdentificationRecord[] = [];

  get records(): readonly ExperimentIdentificationRecord[] {
    return Object.freeze([...this.entries]);
  }

  get identificationRecords(): readonly ExperimentIdentificationRecord[] {
    return Object.freeze(this.entries.filter((entry) => entry.outcome === "completed"));
  }

  get nextRole(): "estimation" | "validation" {
    return this.identificationRecords.length === 0 ? "estimation" : "validation";
  }

  record(
    plan: ControlExperimentPlan,
    terminal: ExperimentIdentificationTerminal,
    samples: readonly ExperimentIdentificationSample[],
    acquisition?: LoopRecordingEvidence
  ): ExperimentIdentificationRecord {
    if (terminal.experiment_id !== plan.id) {
      throw new Error("Terminal Test evidence does not match the reviewed plan.");
    }
    if (this.entries.some((entry) => entry.experimentId === plan.id)) {
      throw new Error("A Test plan can only create one immutable terminal record.");
    }
    if (!["completed", "aborted", "failed", "failed_to_restore"].includes(terminal.status) ||
        !["restored", "not_required", "failed"].includes(terminal.restoration)) {
      throw new Error("Test evidence is not terminal and cannot be handed to Identify.");
    }
    const qualified = terminal.status === "completed" && samples.every(sample => Number.isFinite(sample.input) && Number.isFinite(sample.output)) && (!acquisition || acquisition.completeness === "complete" &&
      acquisition.method === "runtime_scan_buffer" && acquisition.missingScanRanges.length === 0 &&
      acquisition.droppedObservedSamples === 0 && acquisition.actualCycleTimingAvailable);
    const entry: ExperimentIdentificationRecord = Object.freeze({
      experimentId: plan.id,
      role: this.nextRole,
      outcome: qualified ? "completed" : "failed",
      qualification: qualified ? "qualified" : "not_qualified",
      plan: freezePlan(plan),
      terminal: Object.freeze({ ...terminal }),
      samples: Object.freeze(samples.map((sample) => Object.freeze({ ...sample }))),
      ...(acquisition ? { acquisition: freezeAcquisition(acquisition) } : {}),
    });
    this.entries.push(entry);
    return entry;
  }

  /** A failed Test may be retried only from a newly reviewed plan identity. */
  retry(failedExperimentId: string, freshPlan: ControlExperimentPlan): ControlExperimentPlan {
    const failed = this.entries.find((entry) => entry.experimentId === failedExperimentId);
    if (!failed || failed.outcome !== "failed") {
      throw new Error("Only a retained failed Test can be retried.");
    }
    if (freshPlan.id === failedExperimentId || this.entries.some((entry) => entry.experimentId === freshPlan.id)) {
      throw new Error("Retry requires a fresh reviewed plan.");
    }
    return freezePlan(freshPlan);
  }
}

/** Failed Test outcomes remain in the ledger, but never become fitting inputs. */
export function identificationRecordForRole(
  records: readonly ExperimentIdentificationRecord[],
  role: "estimation" | "validation"
): ExperimentIdentificationRecord | undefined {
  return records.find((record) => record.outcome === "completed" && record.role === role);
}

/**
 * Converts the runtime-owned acquisition for one Test into the immutable
 * input consumed by Identify. Editor-observed monitor samples deliberately
 * cannot cross this boundary: they do not establish PLC-cycle completeness.
 */
export function identificationSamplesFromRuntimeRecording(
  plan: Pick<ControlExperimentPlan, "kind">,
  recording: LoopRecording
): ExperimentIdentificationSample[] {
  const acquisition = recording.evidence;
  if (acquisition.method !== "runtime_scan_buffer" || acquisition.completeness !== "complete" ||
      !acquisition.actualCycleTimingAvailable || acquisition.missingScanRanges.length !== 0 ||
      acquisition.droppedObservedSamples !== 0) {
    throw new Error("A complete runtime-owned recording is required before Test data can reach Identify.");
  }
  const samples = recording.rawSamples ?? [];
  if (samples.length < 2) throw new Error("A complete runtime-owned recording requires at least two samples.");
  for (let index = 1; index < samples.length; index += 1) {
    if (samples[index].scan !== samples[index - 1].scan + 1) {
      throw new Error("A complete runtime-owned recording requires contiguous runtime scans.");
    }
  }
  return mapRuntimeRecordingSamples(plan, samples, true);
}

/** Retains received runtime rows for a failed Test without granting Identify qualification. */
export function retainedIdentificationSamplesFromRuntimeRecording(
  plan: Pick<ControlExperimentPlan, "kind">,
  recording: LoopRecording
): ExperimentIdentificationSample[] {
  return mapRuntimeRecordingSamples(plan, recording.rawSamples ?? [], false);
}

function mapRuntimeRecordingSamples(plan: Pick<ControlExperimentPlan, "kind">,
  samples: readonly NonNullable<LoopRecording["rawSamples"]>[number][], requireFinite: boolean): ExperimentIdentificationSample[] {
  return samples.map((sample) => {
    const selected = identificationInput(plan, sample);
    const input = selected.value;
    const output = sample.pv;
    if (input === null || output === null || !Number.isFinite(input) || !Number.isFinite(output)) {
      if (requireFinite) throw new Error("A complete runtime-owned recording contains a missing input or output sample.");
    }
    return {
      timeSeconds: sample.elapsedSeconds,
      input: typeof input === "number" && Number.isFinite(input) ? input : Number.NaN,
      output: typeof output === "number" && Number.isFinite(output) ? output : Number.NaN,
      quality: sample.quality === "good" && selected.quality === "good" && Number.isFinite(input) && Number.isFinite(output)
        ? "good" as const : "bad" as const,
      inputUnit: selected.unit,
      outputUnit: sample.units.pv,
      ...(selected.isManipulatedOutput && (input === sample.outputLimits.minimum || input === sample.outputLimits.maximum)
        ? { saturated: true } : {}),
    };
  });
}

function freezeAcquisition(acquisition: LoopRecordingEvidence): Readonly<LoopRecordingEvidence> {
  return Object.freeze({ ...acquisition, missingScanRanges: Object.freeze(acquisition.missingScanRanges.map(range => Object.freeze({ ...range }))) });
}

function identificationInput(plan: Pick<ControlExperimentPlan, "kind">,
  sample: NonNullable<LoopRecording["rawSamples"]>[number]): {
    value: number | null; unit: string; quality: "good" | "forced" | "bad"; isManipulatedOutput: boolean;
  } {
  if (plan.kind === "setpoint_step") {
    return { value: sample.sp, unit: sample.units.sp, quality: sample.quality, isManipulatedOutput: false };
  }
  if (plan.kind === "load_step") {
    const channels = (sample.additional ?? []).filter(channel => channel.role === "disturbance");
    if (channels.length !== 1) throw new Error("A load-step Test recording requires exactly one declared disturbance input.");
    return { value: channels[0].value, unit: channels[0].unit, quality: channels[0].quality, isManipulatedOutput: false };
  }
  return { value: sample.appliedMv, unit: sample.units.mv, quality: sample.quality, isManipulatedOutput: true };
}

export type ControlExperimentPlanResult =
  | { kind: "accepted"; plan: ControlExperimentPlan }
  | { kind: "rejected"; reason: string };

type AutomaticExperimentSelection =
  | { kind: Exclude<ControlExperimentKind, "observation" | "project_sequence">; amplitude: number }
  | { kind: "rejected"; reason: string };

type AvailableLoopSample = Extract<LoopMonitorSample, { availability: "available" }>;
type ManualCommandBinding = { symbol: string; address: string; iecType: "REAL" | "LREAL" };

/** Uses declared range only for the initial form value; live headroom is checked before review. */
function automaticDefaultAmplitude(loop: ControlLoopDefinition): number {
  const range = loop.signals.setpoint?.range ?? {
    minimum: loop.limits.outputMinimum,
    maximum: loop.limits.outputMaximum,
  };
  return Math.abs(range.maximum - range.minimum) * 0.1;
}

/** Resolves the automatic request to the least intrusive applicable excitation. */
function automaticExperimentSelection(loop: ControlLoopDefinition, sample: AvailableLoopSample,
  requestedAmplitude: number): AutomaticExperimentSelection {
  const mode = sample.controller.mode;
  const kind = mode === "Auto" && sample.setpoint && loop.signals.setpoint
    ? "setpoint_step"
    : mode === "Manual" || loop.topology === "open-loop"
      ? "mv_step"
      : loop.signals.additional?.some(candidate => candidate.role === "disturbance" && candidate.direction === "write")
        ? "load_step"
        : undefined;
  if (!kind) {
    return automaticRejected("Automatic Test selection needs an Auto setpoint path, a Manual output path, or a declared writeable disturbance.");
  }
  const signal = kind === "setpoint_step" ? sample.setpoint!
    : kind === "load_step" ? (sample as any).additional?.find((candidate: any) =>
      loop.signals.additional?.some(binding => binding.id === candidate.id && binding.role === "disturbance" && binding.direction === "write"))
      : sample.manipulatedVariable;
  const binding = kind === "setpoint_step" ? loop.signals.setpoint!
    : kind === "load_step" ? loop.signals.additional?.find(candidate => candidate.role === "disturbance" && candidate.direction === "write")
      : loop.signals.manipulatedVariable;
  if (!signal || !binding || !Number.isFinite(signal.value)) {
    return automaticRejected("Automatic Test selection needs a current declared signal to excite.");
  }
  const minimum = kind === "mv_step" ? loop.limits.outputMinimum : binding.range.minimum;
  const maximum = kind === "mv_step" ? loop.limits.outputMaximum : binding.range.maximum;
  const requestedMagnitude = Math.abs(requestedAmplitude);
  if (!Number.isFinite(requestedMagnitude) || requestedMagnitude <= 0 || maximum <= minimum) {
    return automaticRejected("Automatic Test selection needs a positive finite amplitude and declared signal range.");
  }
  const increase = maximum - signal.value;
  const decrease = signal.value - minimum;
  const signedAmplitude = increase >= decrease
    ? Math.min(requestedMagnitude, increase * 0.5)
    : -Math.min(requestedMagnitude, decrease * 0.5);
  if (!Number.isFinite(signedAmplitude) || signedAmplitude === 0) {
    return automaticRejected("Automatic Test selection has no safe excitation headroom at the current operating point.");
  }
  return { kind, amplitude: signedAmplitude };
}

function automaticRejected(reason: string): AutomaticExperimentSelection {
  return { kind: "rejected", reason };
}

function manualCommandBinding(loop: ControlLoopDefinition): ManualCommandBinding | undefined {
  const { manualOutputSignal: symbol, manualOutputAddress: address, manualOutputType: iecType } = loop.controller;
  return symbol && address && (iecType === "REAL" || iecType === "LREAL") ? { symbol, address, iecType } : undefined;
}

interface PlanInput {
  loop: ControlLoopDefinition;
  sample: LoopMonitorSample;
  kind: ControlExperimentRequestKind;
  amplitude: number;
  durationSeconds: number;
  physicalTopologyReviewed?: boolean;
  seed?: number;
}

export function buildControlExperimentPlan(input: PlanInput): ControlExperimentPlanResult {
  if (!Number.isFinite(input.durationSeconds) || input.durationSeconds < 1 || input.durationSeconds > 3600) {
    return rejected("Duration must be between 1 and 3600 seconds.");
  }
  if (input.sample.availability !== "available") {
    return rejected("Fresh synchronized SP, PV, and MV are required.");
  }
  const activation = input.loop.experiment?.execution === "project-scripted"
    ? input.loop.experiment.activation : undefined;
  if (activation) {
    const primary = [input.sample.setpoint, input.sample.processVariable, input.sample.manipulatedVariable].filter(signal => signal !== undefined);
    if (primary.some(signal => signal.quality === "forced")) {
      return rejected("Remove forcing from SP, PV, and MV before starting a new Test.");
    }
    if (primary.some(signal => signal.quality !== "good")) {
      return rejected("Fresh good-quality SP, PV, and MV are required.");
    }
    if (input.sample.source === "Physical" && !input.physicalTopologyReviewed) {
      return rejected("A reviewed physical topology is required before approval can be requested.");
    }
    const durationMs = Math.round(input.durationSeconds * 1_000);
    const baselineSeconds = input.loop.experiment?.baselineSeconds ?? 0;
    const baselineMs = Math.ceil(baselineSeconds * 1_000 / input.loop.observedSampleTiming.nominalPeriodMs) * input.loop.observedSampleTiming.nominalPeriodMs;
    if (!Number.isFinite(baselineSeconds) || baselineSeconds < 0 ||
        baselineMs + input.loop.observedSampleTiming.nominalPeriodMs > durationMs) {
      return rejected("Maximum duration must include the declared baseline and at least one experiment sample.");
    }
    const abort = input.loop.experiment?.abort;
    return {
      kind: "accepted",
      plan: {
        id: `experiment:${input.loop.id}:${input.sample.scan}:project_sequence`,
        source: input.sample.source === "Physical" ? "physical" : "simulation",
        kind: "project_sequence",
        affectedSignal: activation.symbol,
        address: activation.address,
        valueType: activation.iecType,
        from: false,
        to: true,
        unit: "authority",
        minimum: 0,
        maximum: 1,
        durationMs,
        expectedScan: input.sample.scan,
        stopOnStaleMs: Math.max(500, input.loop.observedSampleTiming.nominalPeriodMs * 5),
        stopConditions: ["Abort", `${input.durationSeconds} s timeout`, "Stale runtime data", "Signal force", "Runtime fault",
          ...(abort ? [`Abort or failure asserts ${abort.symbol} = ${abort.assertedValue}; normal completion does not.`] : [])],
        ...(abort ? { abortBinding: { symbol: abort.symbol, address: abort.address, valueType: abort.iecType, assertedValue: abort.assertedValue } } : {}),
        restorationValue: false,
        requiresPhysicalApproval: input.sample.source === "Physical",
        trajectory: baselineMs > 0 ? [{ atMs: 0, value: 0 }, { atMs: baselineMs, value: 1 }] : [],
      },
    };
  }
  const automatic = input.kind === "automatic";
  const selection: AutomaticExperimentSelection | { kind: ControlExperimentKind; amplitude: number } = automatic
    ? automaticExperimentSelection(input.loop, input.sample, input.amplitude)
    : { kind: input.kind as ControlExperimentKind, amplitude: input.amplitude };
  if (selection.kind === "rejected") return selection;
  const kind = selection.kind;
  const amplitude = selection.amplitude;
  if (!Number.isFinite(amplitude) || (kind !== "observation" && amplitude === 0) ||
      !Number.isFinite(input.durationSeconds) || input.durationSeconds < 1 || input.durationSeconds > 3_600) {
    return rejected("Amplitude must be non-zero and duration must be between 1 and 3600 seconds.");
  }
  const mvExcitation = ["output_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine"].includes(kind);
  // A closed-loop controller must be placed in Manual before its output is
  // excited. An explicitly open-loop experiment owns the output directly, so
  // inventing a controller-mode signal would misrepresent the real topology.
  if (mvExcitation && input.loop.topology !== "open-loop" && input.sample.controller.mode !== "Manual") {
    return rejected("Output steps require confirmed Manual controller mode.");
  }
  const manualCommand = mvExcitation && input.loop.topology !== "open-loop"
    ? manualCommandBinding(input.loop) : undefined;
  if (mvExcitation && input.loop.topology !== "open-loop" && input.loop.controller.manualOutputSignal && !manualCommand) {
    return rejected("Output Test requires the declared Manual-command address and REAL or LREAL type.");
  }
  const needsAutomaticController = kind === "setpoint_step" ||
    (kind === "load_step" && input.loop.topology !== "open-loop");
  if (needsAutomaticController && input.sample.controller.mode !== "Auto") {
    return rejected("Setpoint steps require confirmed Auto controller mode.");
  }
  if (kind === "setpoint_step" && (!input.sample.setpoint || !input.loop.signals.setpoint)) {
    return rejected("A setpoint step requires a declared closed-loop setpoint signal.");
  }
  if (input.sample.source === "Physical" && !input.physicalTopologyReviewed) {
    return rejected("A reviewed physical topology is required before approval can be requested.");
  }

  const disturbanceBinding = input.loop.signals.additional?.find((candidate) =>
    candidate.role === "disturbance" && candidate.direction === "write");
  const disturbanceSignal = (input.sample as any).additional?.find((candidate: any) =>
    candidate.id === disturbanceBinding?.id);
  if (kind === "load_step" && (!disturbanceBinding || !disturbanceSignal)) {
    return rejected("A current declared writeable disturbance signal is required.");
  }
  const signal = kind === "observation" ? input.sample.processVariable
    : kind === "setpoint_step" ? input.sample.setpoint!
      : kind === "load_step" ? disturbanceSignal : input.sample.manipulatedVariable;
  const binding = kind === "observation" ? input.loop.signals.processVariable
    : kind === "setpoint_step" ? input.loop.signals.setpoint!
      : kind === "load_step" ? disturbanceBinding! : input.loop.signals.manipulatedVariable;
  const minimum = mvExcitation ? input.loop.limits.outputMinimum : binding.range.minimum;
  const maximum = mvExcitation ? input.loop.limits.outputMaximum : binding.range.maximum;
  if (signal.quality === "forced") return rejected("The affected signal is forced.");
  if ([input.sample.setpoint, input.sample.processVariable, input.sample.manipulatedVariable]
    .filter((candidate): candidate is NonNullable<typeof candidate> => candidate !== undefined)
    .some((candidate) => candidate.quality !== "good" && candidate.quality !== "forced")) {
    return rejected("Fresh good-quality SP, PV, and MV are required.");
  }
  const requested = signal.value + (kind === "observation" ? 0 : amplitude);
  if (requested < minimum || requested > maximum) {
    return rejected(`The requested value ${requested} ${signal.unit} is outside ${minimum}…${maximum} ${signal.unit}.`);
  }
  const durationMs = Math.round(input.durationSeconds * 1_000);
  const relayLowOutput = signal.value - amplitude;
  if (kind === "relay" && (relayLowOutput < minimum || relayLowOutput > maximum)) {
    return rejected(`The requested relay value ${relayLowOutput} ${signal.unit} is outside ${minimum}…${maximum} ${signal.unit}.`);
  }
  const trajectory = automatic
    ? automaticExperimentTrajectory(kind, signal.value, amplitude, durationMs, input.loop.observedSampleTiming.nominalPeriodMs)
    : experimentTrajectory(kind, signal.value, amplitude, durationMs,
      input.loop.observedSampleTiming.nominalPeriodMs, input.seed ?? 1);
  if (!trajectory) {
    return rejected("Automatic Test duration is too short to capture baseline, held step and restoration.");
  }
  if (trajectory.some((point) => point.value < minimum || point.value > maximum)) {
    return rejected("The generated excitation exceeds the declared signal range.");
  }
  const frequenciesHz = kind === "multisine"
    ? [1 / input.durationSeconds, 2 / input.durationSeconds, 3 / input.durationSeconds] : undefined;
  const pvRange = input.loop.signals.processVariable.range;
  const relayHysteresis = (pvRange.maximum - pvRange.minimum) * 0.005;
  const relayFeedback: ControlRelayFeedbackPlan | undefined = kind === "relay" ? {
    processVariable: input.sample.processVariable.symbol,
    address: input.sample.processVariable.address,
    valueType: input.loop.signals.processVariable.iecType,
    unit: input.sample.processVariable.unit,
    center: input.sample.processVariable.value,
    hysteresis: relayHysteresis,
    lowerThreshold: input.sample.processVariable.value - relayHysteresis,
    upperThreshold: input.sample.processVariable.value + relayHysteresis,
    lowOutput: relayLowOutput,
    highOutput: requested,
  } : undefined;
  return {
    kind: "accepted",
    plan: {
        id: `experiment:${input.loop.id}:${input.sample.scan}:${kind}`,
        source: input.sample.source === "Physical" ? "physical" : "simulation",
      kind,
      affectedSignal: manualCommand?.symbol ?? signal.symbol,
      address: manualCommand?.address ?? signal.address,
      valueType: manualCommand?.iecType ?? binding.iecType,
      from: signal.value,
      to: requested,
      unit: signal.unit,
      minimum,
      maximum,
      durationMs,
      expectedScan: input.sample.scan,
      stopOnStaleMs: Math.max(500, input.loop.observedSampleTiming.nominalPeriodMs * 5),
      stopConditions: [
        "Abort",
        `${input.durationSeconds} s timeout`,
        "Stale runtime data",
        "Signal force",
        "Runtime fault",
      ],
      restorationValue: signal.value,
      requiresPhysicalApproval: input.sample.source === "Physical",
      trajectory,
      ...(relayFeedback ? { relayFeedback } : {}),
      ...(kind === "prbs" ? {
        intervalMs: Math.max(input.loop.observedSampleTiming.nominalPeriodMs * 10, 500),
      } : {}),
      ...(kind === "prbs" ? { seed: (input.seed ?? 1) >>> 0 || 1 } : {}),
      ...(frequenciesHz ? { frequenciesHz } : {}),
    },
  };
}

function experimentTrajectory(kind: ControlExperimentKind, from: number, amplitude: number,
  durationMs: number, samplePeriodMs: number, requestedSeed: number): ControlExperimentPoint[] {
  if (kind === "observation") return [];
  if (["setpoint_step", "load_step", "mv_step", "output_step"].includes(kind)) {
    return [{ atMs: 0, value: from + amplitude }];
  }
  if (kind === "mv_pulse") {
    return [
      { atMs: 0, value: from + amplitude },
      { atMs: Math.floor(durationMs / 2), value: from },
    ];
  }
  if (kind === "relay") return [];
  const intervalMs = kind === "multisine" ? Math.max(samplePeriodMs, 50)
    : Math.max(samplePeriodMs * 10, 500);
  if (kind === "prbs") {
    let state = requestedSeed >>> 0 || 1;
    return Array.from({ length: Math.ceil(durationMs / intervalMs) }, (_, index) => {
      state ^= state << 13; state ^= state >>> 17; state ^= state << 5;
      return { atMs: index * intervalMs, value: from + ((state >>> 0) & 1 ? amplitude : -amplitude) };
    });
  }
  const durationSeconds = durationMs / 1000;
  return Array.from({ length: Math.ceil(durationMs / intervalMs) }, (_, index) => {
    const atMs = index * intervalMs;
    const time = atMs / 1000;
    const wave = [1, 2, 3].reduce((sum, harmonic) =>
      sum + Math.sin(2 * Math.PI * harmonic * time / durationSeconds), 0) / 3;
    return { atMs, value: from + amplitude * wave };
  });
}

/** Automatic identification needs a measured baseline before the held step. */
function automaticExperimentTrajectory(kind: ControlExperimentKind, from: number, amplitude: number,
  durationMs: number, samplePeriodMs: number): ControlExperimentPoint[] | undefined {
  if (!["setpoint_step", "load_step", "mv_step"].includes(kind)) {
    throw new Error(`Automatic Test selection resolved unsupported ${kind} excitation.`);
  }
  const onsetMs = Math.max(samplePeriodMs * 3, Math.round(durationMs * 0.1 / samplePeriodMs) * samplePeriodMs);
  const restorationMs = Math.floor(durationMs / 2);
  if (onsetMs >= restorationMs) {
    return undefined;
  }
  return [{ atMs: 0, value: from }, { atMs: onsetMs, value: from + amplitude }, { atMs: restorationMs, value: from }];
}

type RuntimeRequest = (operation: string, params: unknown) => Promise<unknown>;

/** Uses the existing runtime-control request function supplied by the panel. */
export class ControlExperimentClient {
  constructor(private readonly request: RuntimeRequest) {}

  start(plan: ControlExperimentPlan, physicalApproved: boolean): Promise<unknown> {
    return this.request("control.experiment.start", {
      experiment_id: plan.id,
      source: plan.source,
      kind: plan.kind,
      address: plan.address,
      value: plan.to,
      restoration_value: plan.restorationValue,
      ...(plan.abortBinding ? { abort_binding: { address: plan.abortBinding.address, value_type: plan.abortBinding.valueType, asserted_value: plan.abortBinding.assertedValue } } : {}),
      value_type: plan.valueType,
      minimum: plan.minimum,
      maximum: plan.maximum,
      duration_ms: plan.durationMs,
      expected_scan: plan.expectedScan,
      stop_on_stale_ms: plan.stopOnStaleMs,
      controller_mode: ["output_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine"].includes(plan.kind)
        ? "Manual" : "Auto",
      trajectory: plan.trajectory.map((point) => ({ at_ms: point.atMs,
        value: plan.kind === "project_sequence" && plan.valueType === "BOOL" && (point.value === 0 || point.value === 1)
          ? Boolean(point.value) : point.value })),
      ...(plan.relayFeedback ? {
        relay_feedback: {
          address: plan.relayFeedback.address,
          value_type: plan.relayFeedback.valueType,
          center: plan.relayFeedback.center,
          hysteresis: plan.relayFeedback.hysteresis,
          low_value: plan.relayFeedback.lowOutput,
          high_value: plan.relayFeedback.highOutput,
        },
      } : {}),
      ...(plan.source === "physical" ? {
        physical_approval: {
          user_approved: physicalApproved,
          topology_reviewed: true,
        },
      } : {}),
    });
  }

  status(experimentId: string): Promise<unknown> {
    return this.request("control.experiment.status", { experiment_id: experimentId });
  }

  abort(experimentId: string): Promise<unknown> {
    return this.request("control.experiment.abort", { experiment_id: experimentId });
  }
}

export type ControlExperimentRuntimePhase =
  | "running" | "awaiting_restoration" | "terminal" | "invalid";

/** Keeps editor recovery active until the runtime has observed restoration. */
export function classifyControlExperimentStatus(value: unknown): ControlExperimentRuntimePhase {
  if (!record(value)) return "invalid";
  const status = value.status;
  const restoration = value.restoration;
  if (status === "running" && (restoration === "pending" || restoration === "not_required")) {
    return "running";
  }
  if (["completed", "aborted", "failed"].includes(String(status))) {
    if (restoration === "pending") return "awaiting_restoration";
    if (restoration === "restored" || restoration === "not_required") return "terminal";
    return "invalid";
  }
  return status === "failed_to_restore" && restoration === "failed" ? "terminal" : "invalid";
}

/** Accepts only an experiment identity bound to the exact loop revision being reopened. */
export function decodeControlExperimentRecovery(
  value: unknown,
  scope: ControlExperimentRecoveryScope
): Pick<ControlExperimentRecovery, "experimentId" | "plan" | "recordingOwner"> | undefined {
  if (!record(value)) return undefined;
  if (value.schemaVersion === 1 && Object.keys(value).sort().join(",") ===
      "experimentId,loopId,loopRevision,projectId,schemaVersion") {
    if (value.projectId !== scope.projectId || value.loopId !== scope.loopId || value.loopRevision !== scope.loopRevision ||
        typeof value.experimentId !== "string" || value.experimentId.length === 0) return undefined;
    return { experimentId: value.experimentId };
  }
  if (value.schemaVersion !== 2 || Object.keys(value).sort().join(",") !==
      "experimentId,loopId,loopRevision,plan,projectId,recordingOwner,schemaVersion") return undefined;
  if (value.projectId !== scope.projectId ||
      value.loopId !== scope.loopId || value.loopRevision !== scope.loopRevision ||
      typeof value.experimentId !== "string" || value.experimentId.length === 0) return undefined;
  const plan = decodeRecoveryPlan(value.plan);
  const recordingOwner = decodeRecoveryRecordingOwner(value.recordingOwner, scope);
  if (!plan || !recordingOwner || plan.id !== value.experimentId ||
      recordingOwner.source !== (plan.source === "simulation" ? "Simulation" : "Physical")) return undefined;
  return { experimentId: value.experimentId, plan, recordingOwner };
}

function decodeRecoveryPlan(value: unknown): ControlExperimentPlan | undefined {
  const required = ["id", "source", "kind", "affectedSignal", "address", "valueType", "from", "to", "unit", "minimum", "maximum",
    "durationMs", "expectedScan", "stopOnStaleMs", "stopConditions", "restorationValue", "requiresPhysicalApproval", "trajectory"];
  const allowed = [...required, "abortBinding", "relayFeedback", "intervalMs", "seed", "frequenciesHz"];
  if (!record(value) || !required.every(key => key in value) || Object.keys(value).some(key => !allowed.includes(key))) return undefined;
  const candidate = value as Record<string, any>;
  if (typeof candidate.id !== "string" || !candidate.id || (candidate.source !== "simulation" && candidate.source !== "physical") ||
      !["observation", "setpoint_step", "load_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine", "output_step", "project_sequence"].includes(String(candidate.kind)) ||
      ![candidate.affectedSignal, candidate.address, candidate.valueType, candidate.unit].every(text) || !numberOrBoolean(candidate.from) || !numberOrBoolean(candidate.to) ||
      ![candidate.minimum, candidate.maximum].every(finite) || candidate.minimum > candidate.maximum || !Number.isSafeInteger(candidate.durationMs) ||
      candidate.durationMs < 1_000 || candidate.durationMs > 3_600_000 || !Number.isSafeInteger(candidate.expectedScan) || candidate.expectedScan < 0 ||
      !Number.isSafeInteger(candidate.stopOnStaleMs) || candidate.stopOnStaleMs <= 0 || !Array.isArray(candidate.stopConditions) ||
      candidate.stopConditions.length === 0 || candidate.stopConditions.length > 16 || !candidate.stopConditions.every(text) ||
      !numberOrBoolean(candidate.restorationValue) || typeof candidate.requiresPhysicalApproval !== "boolean" ||
      !Array.isArray(candidate.trajectory) || candidate.trajectory.length > 72_000 || !candidate.trajectory.every((point: unknown) => {
        if (!record(point)) return false;
        const timed = point as Record<string, any>;
        return Number.isSafeInteger(timed.atMs) && timed.atMs >= 0 && timed.atMs <= candidate.durationMs && finite(timed.value);
      }) || !optionalAbortBinding(candidate.abortBinding) || !optionalRelayFeedback(candidate.relayFeedback) ||
      !(candidate.intervalMs === undefined || Number.isSafeInteger(candidate.intervalMs) && candidate.intervalMs > 0) ||
      !(candidate.seed === undefined || Number.isSafeInteger(candidate.seed) && candidate.seed > 0 && candidate.seed <= 0xffffffff) ||
      !(candidate.frequenciesHz === undefined || Array.isArray(candidate.frequenciesHz) && candidate.frequenciesHz.length > 0 && candidate.frequenciesHz.length <= 64 && candidate.frequenciesHz.every(finite))) return undefined;
  return structuredClone(candidate) as unknown as ControlExperimentPlan;
}

function decodeRecoveryRecordingOwner(value: unknown, scope: ControlExperimentRecoveryScope): RecordingRecoveryOwner | undefined {
  if (!record(value) || value.schemaVersion !== 1 || !text(value.recordingId) || value.projectId !== scope.projectId ||
      value.loopId !== scope.loopId || value.loopRevision !== scope.loopRevision || !text(value.runtimeId) ||
      (value.source !== "Simulation" && value.source !== "Physical") || !(value.programRevision === undefined || text(value.programRevision)) ||
      !(value.startAcknowledgementUnconfirmed === undefined || value.startAcknowledgementUnconfirmed === true) ||
      !Array.isArray(value.bindings) || value.bindings.length === 0 || value.bindings.length > 32 || !value.bindings.every(recordingBinding)) return undefined;
  return structuredClone(value) as unknown as RecordingRecoveryOwner;
}

function optionalAbortBinding(value: unknown): boolean {
  return value === undefined || record(value) && text(value.symbol) && text(value.address) && text(value.valueType) && typeof value.assertedValue === "boolean";
}

function optionalRelayFeedback(value: unknown): boolean {
  if (value === undefined) return true;
  if (!record(value)) return false;
  const candidate = value as Record<string, any>;
  return [candidate.processVariable, candidate.address, candidate.valueType, candidate.unit].every(text) &&
    [candidate.center, candidate.hysteresis, candidate.lowerThreshold, candidate.upperThreshold, candidate.lowOutput, candidate.highOutput].every(finite) &&
    candidate.hysteresis > 0 && candidate.lowerThreshold < candidate.upperThreshold;
}

function recordingBinding(value: unknown): boolean {
  return record(value) && text(value.id) && text(value.iec_type) && (value.address === undefined || text(value.address)) &&
    (value.symbol === undefined || text(value.symbol)) && !(value.address === undefined && value.symbol === undefined);
}

function text(value: unknown): value is string { return typeof value === "string" && value.length > 0; }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function numberOrBoolean(value: unknown): boolean { return finite(value) || typeof value === "boolean"; }

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function freezePlan(plan: ControlExperimentPlan): ControlExperimentPlan {
  return Object.freeze({
    ...plan,
    stopConditions: Object.freeze([...plan.stopConditions]),
    trajectory: Object.freeze(plan.trajectory.map((point) => Object.freeze({ ...point }))),
    ...(plan.abortBinding ? { abortBinding: Object.freeze({ ...plan.abortBinding }) } : {}),
    ...(plan.relayFeedback ? { relayFeedback: Object.freeze({ ...plan.relayFeedback }) } : {}),
    ...(plan.frequenciesHz ? { frequenciesHz: Object.freeze([...plan.frequenciesHz]) } : {}),
  }) as ControlExperimentPlan;
}

function rejected(reason: string): ControlExperimentPlanResult {
  return { kind: "rejected", reason };
}
