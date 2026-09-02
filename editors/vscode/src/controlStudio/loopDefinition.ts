export type ControlSignalDirection = "read" | "write";

export interface ControlSignalBinding {
  symbol: string;
  address: string;
  iecType: string;
  direction: ControlSignalDirection;
  unit: string;
  range: { minimum: number; maximum: number };
}

export type ControlAuxiliarySignalRole =
  | "SP" | "PV" | "MV" | "actuator"
  | "disturbance" | "state" | "quality" | "schedule";

export interface ControlAuxiliarySignalBinding extends ControlSignalBinding {
  id: string;
  channel: string;
  role: ControlAuxiliarySignalRole;
}

export interface ControlLoopDefinition {
  schemaVersion: 1;
  topology?: "closed-loop" | "open-loop";
  id: string;
  name: string;
  revision: string;
  projectId: string;
  runtimeId: string;
  controller: {
    instance: string;
    type: string;
    design?: "pid" | "project";
    revision: string;
    action: "direct" | "reverse";
    taskPeriodMs: number;
    modeSignal?: string;
    modeWhenTrue?: "Auto" | "Manual";
    trackingSignal?: string;
  };
  signals: {
    setpoint?: ControlSignalBinding;
    processVariable: ControlSignalBinding;
    manipulatedVariable: ControlSignalBinding;
    actuator?: ControlSignalBinding;
    additional?: ReadonlyArray<ControlAuxiliarySignalBinding>;
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
  experiment?: {
    execution: "control-studio" | "project-scripted";
    kind: "observation" | "setpoint_step" | "load_step" | "mv_step" | "mv_pulse" |
      "relay" | "prbs" | "multisine";
    amplitude: number;
    durationSeconds: number;
    seed: number;
    activation?: {
      symbol: string;
      address: string;
      iecType: "BOOL";
      direction: "write";
    };
  };
  plantId?: string;
  operatingPoint?: string;
}

type PrimarySignalName = "setpoint" | "processVariable" | "manipulatedVariable" | "actuator";

export type ControlLoopDefinitionResult =
  | { kind: "accepted"; loop: ControlLoopDefinition }
  | { kind: "rejected"; reason: "malformed_loop" | "unsupported_version" };

export interface ControlLoopScope {
  projectId: string;
  runtimeId: string;
}

export type SharedPidDesignAvailability =
  | { available: true }
  | { available: false; reason: string };

/** Keeps the shared PID designer from being presented as the owner of project-defined control logic. */
export function sharedPidDesignAvailability(
  loop: ControlLoopDefinition
): SharedPidDesignAvailability {
  if ((loop.controller.design ?? "pid") === "pid") return { available: true };
  return {
    available: false,
    reason: `${loop.controller.type} is designed by the runnable project; shared Control.PID tuning is unavailable.`,
  };
}

export type ControlLoopValidationResult =
  | { kind: "accepted"; loop: ControlLoopDefinition }
  | { kind: "rejected"; reason: "malformed_loop" | "unsupported_version" }
  | { kind: "rejected"; reason: "cross_runtime_loop" }
  | { kind: "rejected"; reason: "incompatible_controller_design" }
  | { kind: "rejected"; reason: "incompatible_topology" }
  | {
      kind: "rejected";
      reason: "invalid_signal_direction" | "incompatible_signal_type";
      signal: keyof ControlLoopDefinition["signals"];
    }
  | {
      kind: "rejected";
      reason: "incompatible_units";
      signals: ["setpoint", "processVariable"];
    }
  | { kind: "rejected"; reason: "duplicate_signal_id" | "duplicate_signal_role"; signal: string }
  | { kind: "rejected"; reason: "ambiguous_signal_address"; signals: [string, string] }
  | { kind: "rejected"; reason: "incompatible_auxiliary_units"; channel: string };

/** Owns the serialized loop boundary before any runtime or UI state is joined. */
export function decodeControlLoopDefinition(value: unknown): ControlLoopDefinitionResult {
  if (!record(value)) return rejected("malformed_loop");
  if (value.schemaVersion !== 1) return rejected("unsupported_version");
  if (
    !nonempty(value.id) || !nonempty(value.name) || !revision(value.revision) ||
    !nonempty(value.projectId) || !nonempty(value.runtimeId) ||
    !controller(value.controller) || !topology(value.topology) || !signals(value.signals, value.topology) ||
    !limits(value.limits) || !timing(value.observedSampleTiming) || !experiment(value.experiment) ||
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
  if ((loop.controller.design ?? "pid") === "pid" && loop.controller.type !== "Control.PID") {
    return { kind: "rejected", reason: "incompatible_controller_design" };
  }
  if (loop.topology === "open-loop" && (loop.controller.design ?? "pid") !== "project") {
    return { kind: "rejected", reason: "incompatible_topology" };
  }

  const expectedDirections: ReadonlyArray<[
    PrimarySignalName,
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

  for (const name of ["setpoint", "processVariable", "manipulatedVariable", "actuator"] as const) {
    const binding = loop.signals[name];
    if (binding && !NUMERIC_IEC_TYPES.has(binding.iecType.toUpperCase())) {
      return { kind: "rejected", reason: "incompatible_signal_type", signal: name };
    }
  }
  if (loop.signals.setpoint && loop.signals.setpoint.unit !== loop.signals.processVariable.unit) {
    return {
      kind: "rejected",
      reason: "incompatible_units",
      signals: ["setpoint", "processVariable"],
    };
  }
  const additional = loop.signals.additional ?? [];
  const ids = new Set<string>(["setpoint", "processVariable", "manipulatedVariable", "actuator"]);
  const channelRoles = new Set<string>();
  const channelUnits = new Map<string, Partial<Record<"SP" | "PV", string>>>();
  for (const binding of additional) {
    if (ids.has(binding.id)) {
      return { kind: "rejected", reason: "duplicate_signal_id", signal: binding.id };
    }
    ids.add(binding.id);
    const channelRole = `${binding.channel}\0${binding.role}`;
    if (channelRoles.has(channelRole)) {
      return { kind: "rejected", reason: "duplicate_signal_role", signal: binding.id };
    }
    channelRoles.add(channelRole);
    const directionValid = binding.role === "disturbance"
      ? binding.direction === "read" || binding.direction === "write"
      : binding.direction === (binding.role === "MV" ? "write" : "read");
    if (!directionValid) {
      return { kind: "rejected", reason: "invalid_signal_direction", signal: "additional" };
    }
    const bindingType = binding.iecType.toUpperCase();
    const booleanEvidence = bindingType === "BOOL" &&
      (binding.role === "quality" || binding.role === "state");
    if (!NUMERIC_IEC_TYPES.has(bindingType) && !booleanEvidence) {
      return { kind: "rejected", reason: "incompatible_signal_type", signal: "additional" };
    }
    if (binding.role === "SP" || binding.role === "PV") {
      const units = channelUnits.get(binding.channel) ?? {};
      units[binding.role] = binding.unit;
      channelUnits.set(binding.channel, units);
    }
  }
  for (const [channel, units] of channelUnits) {
    if (units.SP !== undefined && units.PV !== undefined && units.SP !== units.PV) {
      return { kind: "rejected", reason: "incompatible_auxiliary_units", channel };
    }
  }
  const boundSignals: Array<[string, ControlSignalBinding]> = [
    ...(loop.signals.setpoint ? [["setpoint", loop.signals.setpoint] as [string, ControlSignalBinding]] : []),
    ["processVariable", loop.signals.processVariable],
    ["manipulatedVariable", loop.signals.manipulatedVariable],
    ...(loop.signals.actuator ? [["actuator", loop.signals.actuator] as [string, ControlSignalBinding]] : []),
    ...additional.map((binding) => [binding.id, binding] as [string, ControlSignalBinding]),
  ];
  for (let index = 0; index < boundSignals.length; index += 1) {
    for (let other = 0; other < index; other += 1) {
      if (addressRangesOverlap(boundSignals[index][1], boundSignals[other][1])) {
        return {
          kind: "rejected",
          reason: "ambiguous_signal_address",
          signals: [boundSignals[other][0], boundSignals[index][0]],
        };
      }
    }
  }
  return { kind: "accepted", loop };
}

const NUMERIC_IEC_TYPES = new Set([
  "SINT", "INT", "DINT", "LINT",
  "USINT", "UINT", "UDINT", "ULINT",
  "REAL", "LREAL",
]);

function controller(value: unknown): boolean {
  return record(value) && nonempty(value.instance) && nonempty(value.type) &&
    (value.design === undefined || value.design === "pid" || value.design === "project") &&
    revision(value.revision) && (value.action === "direct" || value.action === "reverse") &&
    positive(value.taskPeriodMs) && optionalString(value.modeSignal) &&
    (value.modeWhenTrue === undefined || value.modeWhenTrue === "Auto" || value.modeWhenTrue === "Manual") &&
    optionalString(value.trackingSignal);
}

function topology(value: unknown): boolean {
  return value === undefined || value === "closed-loop" || value === "open-loop";
}

function signals(value: unknown, declaredTopology: unknown): boolean {
  if (!record(value)) return false;
  const openLoop = declaredTopology === "open-loop";
  return (openLoop ? value.setpoint === undefined : signal(value.setpoint)) &&
    signal(value.processVariable) &&
    signal(value.manipulatedVariable) && (value.actuator === undefined || signal(value.actuator)) &&
    (value.additional === undefined || (Array.isArray(value.additional) && value.additional.every(auxiliarySignal)));
}

function auxiliarySignal(value: unknown): boolean {
  return record(value) && signal(value) && nonempty(value.id) && nonempty(value.channel) &&
    ["SP", "PV", "MV", "actuator", "disturbance", "state", "quality", "schedule"].includes(String(value.role));
}

function signal(value: unknown): boolean {
  return record(value) && nonempty(value.symbol) && nonempty(value.address) &&
    addressRange(value.address as string) !== undefined &&
    nonempty(value.iecType) && (value.direction === "read" || value.direction === "write") &&
    typeof value.unit === "string" && range(value.range);
}

function range(value: unknown): boolean {
  return record(value) && finite(value.minimum) && finite(value.maximum) && value.minimum < value.maximum;
}

function addressRangesOverlap(left: ControlSignalBinding, right: ControlSignalBinding): boolean {
  const leftRange = addressRange(left.address);
  const rightRange = addressRange(right.address);
  if (!leftRange || !rightRange || leftRange.area !== rightRange.area) return left.address === right.address;
  return leftRange.startBit < rightRange.endBit && rightRange.startBit < leftRange.endBit;
}

function addressRange(address: string): { area: string; startBit: number; endBit: number } | undefined {
  const match = /^%([IQM])([XBWDL])(\d+)(?:\.(\d+))?$/.exec(address.toUpperCase());
  if (!match) return undefined;
  const byte = Number(match[3]);
  const bit = match[2] === "X" ? Number(match[4] ?? "0") : 0;
  if (!Number.isSafeInteger(byte) || !Number.isSafeInteger(bit) || bit < 0 || bit > 7) return undefined;
  const width = ({ X: 1, B: 8, W: 16, D: 32, L: 64 } as const)[match[2] as "X" | "B" | "W" | "D" | "L"];
  const startBit = byte * 8 + bit;
  return { area: match[1], startBit, endBit: startBit + width };
}

function limits(value: unknown): boolean {
  return record(value) && finite(value.outputMinimum) && finite(value.outputMaximum) &&
    value.outputMinimum < value.outputMaximum && optionalNonnegative(value.rateUpPerS) &&
    optionalNonnegative(value.rateDownPerS);
}

function timing(value: unknown): boolean {
  return record(value) && positive(value.nominalPeriodMs) && optionalNonnegative(value.maximumJitterMs);
}

function experiment(value: unknown): boolean {
  if (value === undefined) return true;
  if (!record(value) || (value.execution !== "control-studio" && value.execution !== "project-scripted") ||
      !["observation", "setpoint_step", "load_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine"]
        .includes(String(value.kind)) ||
      !finite(value.amplitude) || !finite(value.durationSeconds) || value.durationSeconds < 1 ||
      value.durationSeconds > 3_600 || typeof value.seed !== "number" || !Number.isSafeInteger(value.seed) ||
      value.seed < 1 || value.seed > 0xffff_ffff) return false;
  if (value.execution === "project-scripted") {
    return value.kind === "observation" && value.amplitude === 0 &&
      (value.activation === undefined || activation(value.activation));
  }
  return value.activation === undefined &&
    (value.kind === "observation" ? value.amplitude === 0 : value.amplitude !== 0);
}

function activation(value: unknown): boolean {
  return record(value) && nonempty(value.symbol) && nonempty(value.address) &&
    addressRange(value.address as string) !== undefined && value.iecType === "BOOL" &&
    value.direction === "write";
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
