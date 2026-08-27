export type ControlSignalDirection = "read" | "write";

export interface ControlSignalBinding {
  symbol: string;
  address: string;
  iecType: string;
  direction: ControlSignalDirection;
  unit: string;
  range: { minimum: number; maximum: number };
}

export interface ControlLoopDefinition {
  schemaVersion: 1;
  id: string;
  name: string;
  revision: string;
  projectId: string;
  runtimeId: string;
  controller: {
    instance: string;
    type: "Control.PID";
    revision: string;
    action: "direct" | "reverse";
    taskPeriodMs: number;
    modeSignal?: string;
    trackingSignal?: string;
  };
  signals: {
    setpoint: ControlSignalBinding;
    processVariable: ControlSignalBinding;
    manipulatedVariable: ControlSignalBinding;
    actuator?: ControlSignalBinding;
  };
  limits: {
    outputMinimum: number;
    outputMaximum: number;
    rateUpPerS?: number;
    rateDownPerS?: number;
  };
  observedSampleTiming: {
    nominalPeriodMs: number;
    maximumJitterMs?: number;
  };
  plantId?: string;
  operatingPoint?: string;
}

export type ControlLoopDefinitionResult =
  | { kind: "accepted"; loop: ControlLoopDefinition }
  | { kind: "rejected"; reason: "malformed_loop" | "unsupported_version" };

export interface ControlLoopScope {
  projectId: string;
  runtimeId: string;
}

export type ControlLoopValidationResult =
  | { kind: "accepted"; loop: ControlLoopDefinition }
  | { kind: "rejected"; reason: "malformed_loop" | "unsupported_version" }
  | { kind: "rejected"; reason: "cross_runtime_loop" }
  | {
      kind: "rejected";
      reason: "invalid_signal_direction" | "incompatible_signal_type";
      signal: keyof ControlLoopDefinition["signals"];
    }
  | {
      kind: "rejected";
      reason: "incompatible_units";
      signals: ["setpoint", "processVariable"];
    };

/** Owns the serialized loop boundary before any runtime or UI state is joined. */
export function decodeControlLoopDefinition(value: unknown): ControlLoopDefinitionResult {
  if (!record(value)) return rejected("malformed_loop");
  if (value.schemaVersion !== 1) return rejected("unsupported_version");
  if (
    !nonempty(value.id) || !nonempty(value.name) || !revision(value.revision) ||
    !nonempty(value.projectId) || !nonempty(value.runtimeId) ||
    !controller(value.controller) || !signals(value.signals) ||
    !limits(value.limits) || !timing(value.observedSampleTiming) ||
    !optionalString(value.plantId) || !optionalString(value.operatingPoint)
  ) return rejected("malformed_loop");
  return { kind: "accepted", loop: value as unknown as ControlLoopDefinition };
}

/**
 * Checks the relationships that cannot be established by decoding fields alone.
 * Rejecting an uncertain binding is safer than guessing which runtime value a
 * controller uses.
 */
export function validateControlLoopDefinition(
  value: unknown,
  scope: ControlLoopScope
): ControlLoopValidationResult {
  const decoded = decodeControlLoopDefinition(value);
  if (decoded.kind === "rejected") return decoded;
  const loop = decoded.loop;
  if (loop.projectId !== scope.projectId || loop.runtimeId !== scope.runtimeId) {
    return { kind: "rejected", reason: "cross_runtime_loop" };
  }

  const expectedDirections: ReadonlyArray<[
    keyof ControlLoopDefinition["signals"],
    ControlSignalDirection
  ]> = [
    ["setpoint", "read"],
    ["processVariable", "read"],
    ["manipulatedVariable", "write"],
    ["actuator", "read"],
  ];
  for (const [name, direction] of expectedDirections) {
    const binding = loop.signals[name];
    if (binding && binding.direction !== direction) {
      return { kind: "rejected", reason: "invalid_signal_direction", signal: name };
    }
  }

  for (const name of Object.keys(loop.signals) as Array<keyof ControlLoopDefinition["signals"]>) {
    const binding = loop.signals[name];
    if (binding && !NUMERIC_IEC_TYPES.has(binding.iecType.toUpperCase())) {
      return { kind: "rejected", reason: "incompatible_signal_type", signal: name };
    }
  }
  if (loop.signals.setpoint.unit !== loop.signals.processVariable.unit) {
    return {
      kind: "rejected",
      reason: "incompatible_units",
      signals: ["setpoint", "processVariable"],
    };
  }
  return { kind: "accepted", loop };
}

const NUMERIC_IEC_TYPES = new Set([
  "SINT", "INT", "DINT", "LINT",
  "USINT", "UINT", "UDINT", "ULINT",
  "REAL", "LREAL",
]);

function controller(value: unknown): boolean {
  return record(value) && nonempty(value.instance) && value.type === "Control.PID" &&
    revision(value.revision) && (value.action === "direct" || value.action === "reverse") &&
    positive(value.taskPeriodMs) && optionalString(value.modeSignal) && optionalString(value.trackingSignal);
}

function signals(value: unknown): boolean {
  return record(value) && signal(value.setpoint) && signal(value.processVariable) &&
    signal(value.manipulatedVariable) && (value.actuator === undefined || signal(value.actuator));
}

function signal(value: unknown): boolean {
  return record(value) && nonempty(value.symbol) && nonempty(value.address) &&
    nonempty(value.iecType) && (value.direction === "read" || value.direction === "write") &&
    typeof value.unit === "string" && range(value.range);
}

function range(value: unknown): boolean {
  return record(value) && finite(value.minimum) && finite(value.maximum) && value.minimum < value.maximum;
}

function limits(value: unknown): boolean {
  return record(value) && finite(value.outputMinimum) && finite(value.outputMaximum) &&
    value.outputMinimum < value.outputMaximum && optionalNonnegative(value.rateUpPerS) &&
    optionalNonnegative(value.rateDownPerS);
}

function timing(value: unknown): boolean {
  return record(value) && positive(value.nominalPeriodMs) && optionalNonnegative(value.maximumJitterMs);
}

function rejected(reason: "malformed_loop" | "unsupported_version"): ControlLoopDefinitionResult {
  return { kind: "rejected", reason };
}

function record(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function nonempty(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function optionalString(value: unknown): boolean {
  return value === undefined || nonempty(value);
}

function revision(value: unknown): value is string {
  return nonempty(value) && value.startsWith("sha256:") && value.length > "sha256:".length;
}

function finite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function positive(value: unknown): value is number {
  return finite(value) && value > 0;
}

function optionalNonnegative(value: unknown): boolean {
  return value === undefined || (finite(value) && value >= 0);
}
