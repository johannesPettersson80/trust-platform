import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMonitorSample } from "./loopMonitor";

export type ControlExperimentKind = "setpoint_step" | "output_step";

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

export interface ControlExperimentPlan {
  id: string;
  source: "simulation" | "physical";
  kind: ControlExperimentKind;
  affectedSignal: string;
  address: string;
  valueType: string;
  from: number;
  to: number;
  unit: string;
  minimum: number;
  maximum: number;
  durationMs: number;
  expectedScan: number;
  stopOnStaleMs: number;
  stopConditions: string[];
  restorationValue: number;
  requiresPhysicalApproval: boolean;
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
}

export function buildControlExperimentPlan(input: PlanInput): ControlExperimentPlanResult {
  if (input.sample.availability !== "available") {
    return rejected("Fresh synchronized SP, PV, and MV are required.");
  }
  if (!Number.isFinite(input.amplitude) || input.amplitude === 0 ||
      !Number.isFinite(input.durationSeconds) || input.durationSeconds < 1 || input.durationSeconds > 300) {
    return rejected("Amplitude must be non-zero and duration must be between 1 and 300 seconds.");
  }
  if (input.kind === "output_step" && input.sample.controller.mode !== "Manual") {
    return rejected("Output steps require confirmed Manual controller mode.");
  }
  if (input.kind === "setpoint_step" && input.sample.controller.mode !== "Auto") {
    return rejected("Setpoint steps require confirmed Auto controller mode.");
  }
  if (input.sample.source === "Physical" && !input.physicalTopologyReviewed) {
    return rejected("A reviewed physical topology is required before approval can be requested.");
  }

  const signal = input.kind === "setpoint_step"
    ? input.sample.setpoint
    : input.sample.manipulatedVariable;
  const binding = input.kind === "setpoint_step"
    ? input.loop.signals.setpoint
    : input.loop.signals.manipulatedVariable;
  const minimum = input.kind === "setpoint_step"
    ? binding.range.minimum
    : input.loop.limits.outputMinimum;
  const maximum = input.kind === "setpoint_step"
    ? binding.range.maximum
    : input.loop.limits.outputMaximum;
  if (signal.quality === "forced") return rejected("The affected signal is forced.");
  const requested = signal.value + input.amplitude;
  if (requested < minimum || requested > maximum) {
    return rejected(`The requested value ${requested} ${signal.unit} is outside ${minimum}…${maximum} ${signal.unit}.`);
  }
  const durationMs = Math.round(input.durationSeconds * 1_000);
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
    },
  };
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
      controller_mode: plan.kind === "output_step" ? "Manual" : "Auto",
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
