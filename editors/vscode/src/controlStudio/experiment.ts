import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMonitorSample } from "./loopMonitor";

export type ControlExperimentKind = "observation" | "setpoint_step" | "load_step" |
  "mv_step" | "mv_pulse" | "relay" | "prbs" | "multisine" | "output_step" |
  "project_sequence";

export interface ControlExperimentChoice {
  value: Exclude<ControlExperimentKind, "output_step">;
  label: string;
}

export interface ControlExperimentDefaults {
  execution: "control-studio" | "project-scripted";
  kind: ControlExperimentChoice["value"];
  amplitude: number;
  durationSeconds: number;
  seed: number;
}

export function controlExperimentDefaults(loop: ControlLoopDefinition): ControlExperimentDefaults {
  return loop.experiment ?? {
    execution: "control-studio",
    kind: "setpoint_step",
    amplitude: 5,
    durationSeconds: 30,
    seed: 1,
  };
}

/** User-facing experiments; output_step remains accepted only as a legacy wire alias. */
export function controlExperimentChoices(): readonly ControlExperimentChoice[] {
  return [
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
  schemaVersion: 1;
  experimentId: string;
  projectId: string;
  loopId: string;
  loopRevision: string;
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
  relayFeedback?: ControlRelayFeedbackPlan;
  intervalMs?: number;
  seed?: number;
  frequenciesHz?: number[];
}

export type ControlExperimentPlanResult =
  | { kind: "accepted"; plan: ControlExperimentPlan }
  | { kind: "rejected"; reason: string };

interface PlanInput {
  loop: ControlLoopDefinition;
  sample: LoopMonitorSample;
  kind: ControlExperimentKind;
  amplitude: number;
  durationSeconds: number;
  physicalTopologyReviewed?: boolean;
  seed?: number;
}

export function buildControlExperimentPlan(input: PlanInput): ControlExperimentPlanResult {
  if (input.sample.availability !== "available") {
    return rejected("Fresh synchronized SP, PV, and MV are required.");
  }
  const activation = input.loop.experiment?.execution === "project-scripted"
    ? input.loop.experiment.activation : undefined;
  if (activation) {
    if (input.sample.source === "Physical" && !input.physicalTopologyReviewed) {
      return rejected("A reviewed physical topology is required before approval can be requested.");
    }
    const durationMs = Math.round(input.durationSeconds * 1_000);
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
        stopConditions: ["Abort", `${input.durationSeconds} s timeout`, "Stale runtime data", "Signal force", "Runtime fault"],
        restorationValue: false,
        requiresPhysicalApproval: input.sample.source === "Physical",
        trajectory: [],
      },
    };
  }
  if (!Number.isFinite(input.amplitude) || (input.kind !== "observation" && input.amplitude === 0) ||
      !Number.isFinite(input.durationSeconds) || input.durationSeconds < 1 || input.durationSeconds > 3_600) {
    return rejected("Amplitude must be non-zero and duration must be between 1 and 3600 seconds.");
  }
  const mvExcitation = ["output_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine"].includes(input.kind);
  // A closed-loop controller must be placed in Manual before its output is
  // excited. An explicitly open-loop experiment owns the output directly, so
  // inventing a controller-mode signal would misrepresent the real topology.
  if (mvExcitation && input.loop.topology !== "open-loop" && input.sample.controller.mode !== "Manual") {
    return rejected("Output steps require confirmed Manual controller mode.");
  }
  const needsAutomaticController = input.kind === "setpoint_step" ||
    (input.kind === "load_step" && input.loop.topology !== "open-loop");
  if (needsAutomaticController && input.sample.controller.mode !== "Auto") {
    return rejected("Setpoint steps require confirmed Auto controller mode.");
  }
  if (input.kind === "setpoint_step" && (!input.sample.setpoint || !input.loop.signals.setpoint)) {
    return rejected("A setpoint step requires a declared closed-loop setpoint signal.");
  }
  if (input.sample.source === "Physical" && !input.physicalTopologyReviewed) {
    return rejected("A reviewed physical topology is required before approval can be requested.");
  }

  const disturbanceBinding = input.loop.signals.additional?.find((candidate) =>
    candidate.role === "disturbance" && candidate.direction === "write");
  const disturbanceSignal = (input.sample as any).additional?.find((candidate: any) =>
    candidate.id === disturbanceBinding?.id);
  if (input.kind === "load_step" && (!disturbanceBinding || !disturbanceSignal)) {
    return rejected("A current declared writeable disturbance signal is required.");
  }
  const signal = input.kind === "observation" ? input.sample.processVariable
    : input.kind === "setpoint_step" ? input.sample.setpoint!
      : input.kind === "load_step" ? disturbanceSignal : input.sample.manipulatedVariable;
  const binding = input.kind === "observation" ? input.loop.signals.processVariable
    : input.kind === "setpoint_step" ? input.loop.signals.setpoint!
      : input.kind === "load_step" ? disturbanceBinding! : input.loop.signals.manipulatedVariable;
  const minimum = mvExcitation ? input.loop.limits.outputMinimum : binding.range.minimum;
  const maximum = mvExcitation ? input.loop.limits.outputMaximum : binding.range.maximum;
  if (signal.quality === "forced") return rejected("The affected signal is forced.");
  if ([input.sample.setpoint, input.sample.processVariable, input.sample.manipulatedVariable]
    .filter((candidate): candidate is NonNullable<typeof candidate> => candidate !== undefined)
    .some((candidate) => candidate.quality !== "good" && candidate.quality !== "forced")) {
    return rejected("Fresh good-quality SP, PV, and MV are required.");
  }
  const requested = signal.value + (input.kind === "observation" ? 0 : input.amplitude);
  if (requested < minimum || requested > maximum) {
    return rejected(`The requested value ${requested} ${signal.unit} is outside ${minimum}…${maximum} ${signal.unit}.`);
  }
  const durationMs = Math.round(input.durationSeconds * 1_000);
  const relayLowOutput = signal.value - input.amplitude;
  if (input.kind === "relay" && (relayLowOutput < minimum || relayLowOutput > maximum)) {
    return rejected(`The requested relay value ${relayLowOutput} ${signal.unit} is outside ${minimum}…${maximum} ${signal.unit}.`);
  }
  const trajectory = experimentTrajectory(input.kind, signal.value, input.amplitude, durationMs,
    input.loop.observedSampleTiming.nominalPeriodMs, input.seed ?? 1);
  if (trajectory.some((point) => point.value < minimum || point.value > maximum)) {
    return rejected("The generated excitation exceeds the declared signal range.");
  }
  const frequenciesHz = input.kind === "multisine"
    ? [1 / input.durationSeconds, 2 / input.durationSeconds, 3 / input.durationSeconds] : undefined;
  const pvRange = input.loop.signals.processVariable.range;
  const relayHysteresis = (pvRange.maximum - pvRange.minimum) * 0.005;
  const relayFeedback: ControlRelayFeedbackPlan | undefined = input.kind === "relay" ? {
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
      id: `experiment:${input.loop.id}:${input.sample.scan}:${input.kind}`,
      source: input.sample.source === "Physical" ? "physical" : "simulation",
      kind: input.kind,
      affectedSignal: signal.symbol,
      address: signal.address,
      valueType: binding.iecType,
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
      ...(input.kind === "prbs" ? {
        intervalMs: Math.max(input.loop.observedSampleTiming.nominalPeriodMs * 10, 500),
      } : {}),
      ...(input.kind === "prbs" ? { seed: (input.seed ?? 1) >>> 0 || 1 } : {}),
      ...(frequenciesHz ? { frequenciesHz } : {}),
    },
  };
}

function experimentTrajectory(kind: ControlExperimentKind, from: number, amplitude: number,
  durationMs: number, samplePeriodMs: number, requestedSeed: number): ControlExperimentPoint[] {
  if (kind === "observation") return [];
  if (["setpoint_step", "load_step", "mv_step", "mv_pulse", "output_step"].includes(kind)) {
    return [{ atMs: 0, value: from + amplitude }];
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
      value_type: plan.valueType,
      minimum: plan.minimum,
      maximum: plan.maximum,
      duration_ms: plan.durationMs,
      expected_scan: plan.expectedScan,
      stop_on_stale_ms: plan.stopOnStaleMs,
      controller_mode: ["output_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine"].includes(plan.kind)
        ? "Manual" : "Auto",
      trajectory: plan.trajectory.map((point) => ({ at_ms: point.atMs, value: point.value })),
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
): Pick<ControlExperimentRecovery, "experimentId"> | undefined {
  if (!record(value) || Object.keys(value).sort().join(",") !==
      "experimentId,loopId,loopRevision,projectId,schemaVersion") return undefined;
  if (value.schemaVersion !== 1 || value.projectId !== scope.projectId ||
      value.loopId !== scope.loopId || value.loopRevision !== scope.loopRevision ||
      typeof value.experimentId !== "string" || value.experimentId.length === 0) return undefined;
  return { experimentId: value.experimentId };
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function rejected(reason: string): ControlExperimentPlanResult {
  return { kind: "rejected", reason };
}
