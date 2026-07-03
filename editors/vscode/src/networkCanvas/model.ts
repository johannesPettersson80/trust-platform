import type { CommCapabilitiesResponse } from "../communication/capability";
import type {
  CommApplyResponse,
  CommConfiguredInstance,
  CommProtocolSchema,
  CommSchemaResponse,
} from "../communication/schemaForm";
import type { FleetTopologyResponse } from "./fleetTopology";
import {
  fleetFaultsFromView,
  fleetViewFromTopology,
  type NetworkCanvasFleetView,
} from "./fleetModel";

export type NetworkCanvasStage =
  | "welcome"
  | "runtime_live"
  | "intent"
  | "add_device"
  | "connected";

export interface NetworkCanvasStep {
  readonly id: NetworkCanvasStage;
  readonly label: string;
}

export type NetworkRuntimeState =
  | "not_started"
  | "starting"
  | "running"
  | "error";

export type NetworkDeviceStatus =
  | "draft"
  | "pending"
  | "connected"
  | "degraded"
  | "error"
  | "preview";

export type NetworkCanvasProtocolId =
  | "modbus_tcp"
  | "mqtt"
  | "ethercat"
  | "gpio"
  | "simulated"
  | "loopback";

export type NetworkCanvasFailureKind =
  | "missing_binary"
  | "port_conflict"
  | "workspace_permission"
  | "failed_spawn"
  | "stale_runtime";

export interface NetworkCanvasFailure {
  readonly kind: NetworkCanvasFailureKind;
  readonly message: string;
  readonly detail?: string;
}

export interface NetworkRuntimeEvidence {
  readonly running: boolean;
  readonly runtimeState: "running" | "connected" | "stopped";
  readonly runtimeMode: "simulate" | "online";
}

export interface NetworkIoEntry {
  readonly name?: string;
  readonly address: string;
  readonly value: string;
}

export interface NetworkIoState {
  readonly inputs: readonly NetworkIoEntry[];
  readonly outputs: readonly NetworkIoEntry[];
  readonly memory: readonly NetworkIoEntry[];
}

export interface NetworkCanvasLiveValue {
  readonly label: string;
  readonly value: string;
  readonly numeric?: number;
}

export interface NetworkCanvasDevice {
  readonly id: string;
  readonly name: string;
  readonly protocol: NetworkCanvasProtocolId;
  readonly protocolTitle: string;
  readonly instanceId?: string;
  readonly status: NetworkDeviceStatus;
  readonly statusText: string;
  readonly liveValues: readonly NetworkCanvasLiveValue[];
  readonly terminalCount: number;
  readonly overflowCount: number;
}

export interface NetworkCanvasEdge {
  readonly id: string;
  readonly from: string;
  readonly to: string;
  readonly role: string;
  readonly status: NetworkDeviceStatus;
  readonly label: string;
}

export interface NetworkCanvasFault {
  readonly id: string;
  readonly label: string;
  readonly targetNodeId: string;
  readonly severity: "warning" | "error";
}

export interface NetworkCanvasDiscoveredDevice {
  readonly id: string;
  readonly label: string;
  readonly protocol: NetworkCanvasProtocolId;
  readonly detail: string;
  readonly params?: Record<string, unknown>;
}

export interface NetworkCanvasTemplate {
  readonly id: string;
  readonly label: string;
  readonly protocol: NetworkCanvasProtocolId;
  readonly description: string;
}

export interface NetworkCanvasModel {
  readonly stage: NetworkCanvasStage;
  readonly steps: readonly NetworkCanvasStep[];
  readonly clickCountFromZero: number;
  readonly activeProtocol: NetworkCanvasProtocolId;
  readonly activeSchema?: CommProtocolSchema;
  readonly schema?: CommSchemaResponse;
  readonly applyResult?: CommApplyResponse;
  readonly searchQuery: string;
  readonly pinnedNodeId?: string;
  readonly quickAddOpen: boolean;
  readonly runtime: {
    readonly name: string;
    readonly hostLabel: string;
    readonly mode: "simulate";
    readonly state: NetworkRuntimeState;
    readonly statusText: string;
  };
  readonly devices: readonly NetworkCanvasDevice[];
  readonly device?: NetworkCanvasDevice;
  readonly edges: readonly NetworkCanvasEdge[];
  readonly faults: readonly NetworkCanvasFault[];
  readonly discoveredDevices: readonly NetworkCanvasDiscoveredDevice[];
  readonly templates: readonly NetworkCanvasTemplate[];
  readonly fleet?: NetworkCanvasFleetView;
  readonly topologyError?: string;
  readonly failure?: NetworkCanvasFailure;
  readonly previewNotice?: string;
  readonly runtimeSetupMessage?: string;
}

export interface BuildNetworkCanvasModelInput {
  readonly stage?: NetworkCanvasStage;
  readonly runtime?: NetworkRuntimeEvidence;
  readonly ioState?: NetworkIoState;
  readonly schema?: CommSchemaResponse;
  readonly capabilities?: CommCapabilitiesResponse;
  readonly activeProtocol?: string;
  readonly applyResult?: CommApplyResponse;
  readonly searchQuery?: string;
  readonly pinnedNodeId?: string;
  readonly quickAddOpen?: boolean;
  readonly discoveredDevices?: readonly NetworkCanvasDiscoveredDevice[];
  readonly topology?: FleetTopologyResponse;
  readonly topologyError?: string;
  readonly starting?: boolean;
  readonly failure?: NetworkCanvasFailure;
  readonly deviceRequested?: boolean;
  readonly previewNotice?: string;
  readonly runtimeSetupMessage?: string;
}

export const NETWORK_CANVAS_STEPS: readonly NetworkCanvasStep[] = [
  { id: "welcome", label: "Welcome" },
  { id: "runtime_live", label: "Runtime live" },
  { id: "intent", label: "What to connect" },
  { id: "add_device", label: "Set up device" },
  { id: "connected", label: "Connected" },
];

const STAGE_ORDER: readonly NetworkCanvasStage[] = NETWORK_CANVAS_STEPS.map(
  (step) => step.id
);

export const NETWORK_CANVAS_IO_PROTOCOLS: readonly NetworkCanvasProtocolId[] = [
  "simulated",
  "loopback",
  "modbus_tcp",
  "mqtt",
  "ethercat",
  "gpio",
];

export const NETWORK_CANVAS_TEMPLATES: readonly NetworkCanvasTemplate[] = [
  {
    id: "template-simulated-counter",
    label: "Local simulated counter",
    protocol: "simulated",
    description: "No hardware. Adds simulated inputs/outputs with a counter pattern.",
  },
  {
    id: "template-loopback",
    label: "Loopback sanity check",
    protocol: "loopback",
    description: "Echoes outputs back into inputs for a fast local wiring check.",
  },
  {
    id: "template-modbus-drive",
    label: "Modbus drive or meter",
    protocol: "modbus_tcp",
    description: "TCP device at 192.168.1.50:502 with Unit ID 1.",
  },
  {
    id: "template-mqtt-broker",
    label: "MQTT broker",
    protocol: "mqtt",
    description: "Broker at 127.0.0.1:1883 with default process topics.",
  },
];

export function buildNetworkCanvasModel(
  input: BuildNetworkCanvasModelInput | NetworkCanvasStage = {}
): NetworkCanvasModel {
  const normalizedInput = typeof input === "string" ? { stage: input } : input;
  const normalizedStage = isNetworkCanvasStage(normalizedInput.stage)
    ? normalizedInput.stage
    : "welcome";
  const activeProtocol = normalizeCanvasProtocol(
    normalizedInput.activeProtocol,
    "simulated"
  );
  const runtimeIsProven =
    normalizedInput.runtime?.running === true &&
    (normalizedInput.runtime.runtimeState === "running" ||
      normalizedInput.runtime.runtimeState === "connected");
  const activeFailure = isNeutralStoppedRuntimeFailure(normalizedInput.failure)
    ? undefined
    : normalizedInput.failure;
  const runtimeState: NetworkRuntimeState = activeFailure
    ? "error"
    : normalizedInput.starting
      ? "starting"
      : runtimeIsProven
        ? "running"
        : "not_started";
  const liveValues = liveValuesFromIoState(normalizedInput.ioState);
  const activeSchema = schemaForProtocol(normalizedInput.schema, activeProtocol);
  const deviceRequested =
    normalizedInput.deviceRequested || normalizedStage === "connected";
  const schemaDevices = devicesFromSchema(
    normalizedInput.schema,
    normalizedInput.capabilities,
    runtimeIsProven,
    liveValues
  );
  const draftDevice =
    normalizedStage === "add_device" && !schemaDevices.some((device) => device.protocol === activeProtocol)
      ? draftDeviceForProtocol(activeProtocol, activeSchema)
      : undefined;
  const firstRunDevice =
    deviceRequested && schemaDevices.length === 0
      ? firstRunSimulatedDevice(runtimeIsProven, liveValues)
      : undefined;
  const devices = filterDevices(
    [draftDevice, firstRunDevice, ...schemaDevices].filter(
      (device): device is NetworkCanvasDevice => Boolean(device)
    ),
    normalizedInput.searchQuery
  );
  const edges = devices.map((device) => edgeForDevice(device));
  const fleet = fleetViewFromTopology(normalizedInput.topology, normalizedInput.searchQuery);
  const faults = faultsForModel(
    devices,
    normalizedInput.applyResult,
    activeFailure,
    fleet
  );

  return {
    stage: normalizedStage,
    steps: NETWORK_CANVAS_STEPS,
    clickCountFromZero: stageIndex(normalizedStage),
    activeProtocol,
    activeSchema,
    schema: normalizedInput.schema,
    applyResult: normalizedInput.applyResult,
    searchQuery: (normalizedInput.searchQuery ?? "").trim(),
    pinnedNodeId: normalizedInput.pinnedNodeId,
    quickAddOpen: normalizedInput.quickAddOpen === true,
    runtime: {
      name: "Local simulator",
      hostLabel: "this computer",
      mode: "simulate",
      state: runtimeState,
      statusText: runtimeStatusText(runtimeState),
    },
    devices,
    device: devices[0],
    edges,
    faults,
    discoveredDevices: normalizedInput.discoveredDevices ?? [],
    templates: NETWORK_CANVAS_TEMPLATES,
    fleet,
    topologyError: normalizedInput.topologyError,
    failure: activeFailure,
    previewNotice: normalizedInput.previewNotice,
    runtimeSetupMessage: normalizedInput.runtimeSetupMessage,
  };
}

export function isNeutralStoppedRuntimeFailure(
  failure: NetworkCanvasFailure | undefined
): boolean {
  if (!failure || failure.kind !== "stale_runtime") {
    return false;
  }
  const text = `${failure.message} ${failure.detail ?? ""}`;
  return (
    /local runtime is stopped|local simulator is stopped/i.test(text) ||
    (/runtime (?:is )?not reachable/i.test(text) && /unix:\/\//i.test(text))
  );
}

export function nextNetworkCanvasStage(
  stage: NetworkCanvasStage
): NetworkCanvasStage {
  const index = stageIndex(stage);
  return STAGE_ORDER[Math.min(index + 1, STAGE_ORDER.length - 1)];
}

export function isNetworkCanvasStage(
  value: unknown
): value is NetworkCanvasStage {
  return typeof value === "string" && STAGE_ORDER.includes(value as NetworkCanvasStage);
}

function stageIndex(stage: NetworkCanvasStage): number {
  const index = STAGE_ORDER.indexOf(stage);
  return index >= 0 ? index : 0;
}

function runtimeStatusText(state: NetworkRuntimeState): string {
  switch (state) {
    case "running":
      return "● Running";
    case "starting":
      return "Starting…";
    case "error":
      return "Needs attention";
    case "not_started":
      return "Not started";
  }
}

function liveValuesFromIoState(
  state: NetworkIoState | undefined
): readonly NetworkCanvasLiveValue[] {
  if (!state) {
    return [];
  }
  return [...state.inputs, ...state.outputs, ...state.memory]
    .slice(0, 12)
    .map((entry) => ({
      label: entry.name || entry.address,
      value: entry.value,
      numeric: numericValue(entry.value),
    }));
}

function normalizeCanvasProtocol(
  value: unknown,
  fallback: NetworkCanvasProtocolId
): NetworkCanvasProtocolId {
  if (typeof value !== "string") {
    return fallback;
  }
  const normalized = value.trim().replace(/-/g, "_").toLowerCase();
  return NETWORK_CANVAS_IO_PROTOCOLS.includes(normalized as NetworkCanvasProtocolId)
    ? (normalized as NetworkCanvasProtocolId)
    : fallback;
}

function schemaForProtocol(
  schema: CommSchemaResponse | undefined,
  protocol: NetworkCanvasProtocolId
): CommProtocolSchema | undefined {
  return schema?.protocols.find((entry) => entry.id === protocol);
}

function devicesFromSchema(
  schema: CommSchemaResponse | undefined,
  capabilities: CommCapabilitiesResponse | undefined,
  runtimeIsProven: boolean,
  liveValues: readonly NetworkCanvasLiveValue[]
): readonly NetworkCanvasDevice[] {
  if (!schema) {
    return [];
  }
  return schema.protocols.flatMap((protocol) => {
    const canvasProtocol = normalizeCanvasProtocol(protocol.id, "simulated");
    if (canvasProtocol !== protocol.id) {
      return [];
    }
    return (protocol.instances ?? []).map((instance) =>
      deviceFromInstance(protocol, instance, capabilities, runtimeIsProven, liveValues)
    );
  });
}

function deviceFromInstance(
  protocol: CommProtocolSchema,
  instance: CommConfiguredInstance,
  capabilities: CommCapabilitiesResponse | undefined,
  runtimeIsProven: boolean,
  liveValues: readonly NetworkCanvasLiveValue[]
): NetworkCanvasDevice {
  const canvasProtocol = normalizeCanvasProtocol(protocol.id, "simulated");
  const capability = capabilities?.capabilities.find(
    (entry) => entry.id === protocol.id
  );
  const connected =
    runtimeIsProven &&
    capability?.health === "connected" &&
    (canvasProtocol === "simulated" || canvasProtocol === "loopback"
      ? liveValues.length > 0
      : true);
  const status: NetworkDeviceStatus = connected
    ? "connected"
    : capability?.health === "error"
      ? "error"
      : capability?.health === "degraded"
        ? "degraded"
        : "pending";
  return {
    id: instance.id,
    name: instance.display_name,
    protocol: canvasProtocol,
    protocolTitle: protocol.title,
    instanceId: instance.id,
    status,
    statusText: deviceStatusText(status),
    liveValues: connected ? liveValues : [],
    terminalCount: Math.max(2, protocol.fields.length),
    overflowCount: Math.max(0, protocol.fields.length - 5),
  };
}

function draftDeviceForProtocol(
  protocol: NetworkCanvasProtocolId,
  schema: CommProtocolSchema | undefined
): NetworkCanvasDevice {
  return {
    id: `draft:${protocol}`,
    name: schema?.title ?? protocolLabel(protocol),
    protocol,
    protocolTitle: schema?.title ?? protocolLabel(protocol),
    status: "draft",
    statusText: "Draft setup",
    liveValues: [],
    terminalCount: Math.max(2, schema?.fields.length ?? 3),
    overflowCount: Math.max(0, (schema?.fields.length ?? 3) - 5),
  };
}

function firstRunSimulatedDevice(
  runtimeIsProven: boolean,
  liveValues: readonly NetworkCanvasLiveValue[]
): NetworkCanvasDevice {
  const connected = runtimeIsProven && liveValues.length > 0;
  return {
    id: "first-run:simulated",
    name: "Drive A",
    protocol: "simulated",
    protocolTitle: "Simulated",
    status: connected ? "connected" : "pending",
    statusText: connected ? "Connected" : "Waiting for runtime I/O",
    liveValues,
    terminalCount: 4,
    overflowCount: 0,
  };
}

function filterDevices(
  devices: readonly NetworkCanvasDevice[],
  searchQuery: string | undefined
): readonly NetworkCanvasDevice[] {
  const query = (searchQuery ?? "").trim().toLowerCase();
  if (query.length === 0) {
    return devices;
  }
  return devices.map((device) => {
    const haystack = `${device.name} ${device.protocolTitle} ${device.statusText}`.toLowerCase();
    return haystack.includes(query)
      ? device
      : { ...device, status: "preview" as const, statusText: "Dimmed by search" };
  });
}

function edgeForDevice(device: NetworkCanvasDevice): NetworkCanvasEdge {
  return {
    id: `edge:runtime:${device.id}`,
    from: "runtime:local",
    to: device.id,
    role: edgeRoleForProtocol(device.protocol),
    status: device.status,
    label: `${device.protocolTitle} · ${edgeRoleForProtocol(device.protocol)}`,
  };
}

function faultsForModel(
  devices: readonly NetworkCanvasDevice[],
  applyResult: CommApplyResponse | undefined,
  failure: NetworkCanvasFailure | undefined,
  fleet: NetworkCanvasFleetView | undefined
): readonly NetworkCanvasFault[] {
  const faults: NetworkCanvasFault[] = [];
  if (failure) {
    faults.push({
      id: `runtime:${failure.kind}`,
      label: failure.message,
      targetNodeId: "runtime:local",
      severity: "error",
    });
  }
  if (applyResult && applyResult.lifecycle_effect === "blocked") {
    faults.push({
      id: `apply:${applyResult.protocol}`,
      label: applyResult.message,
      targetNodeId: `draft:${normalizeCanvasProtocol(applyResult.protocol, "simulated")}`,
      severity: "error",
    });
  }
  for (const device of devices) {
    if (device.status === "degraded" || device.status === "error") {
      faults.push({
        id: `device:${device.id}`,
        label: `${device.name}: ${device.statusText}`,
        targetNodeId: device.id,
        severity: device.status === "error" ? "error" : "warning",
      });
    }
  }
  faults.push(...fleetFaultsFromView(fleet));
  return faults;
}

function edgeRoleForProtocol(protocol: NetworkCanvasProtocolId): string {
  switch (protocol) {
    case "mqtt":
      return "runtime publisher/subscriber";
    case "simulated":
    case "loopback":
      return "runtime local driver";
    default:
      return "runtime client";
  }
}

function deviceStatusText(status: NetworkDeviceStatus): string {
  switch (status) {
    case "connected":
      return "Connected";
    case "degraded":
      return "Degraded";
    case "error":
      return "Error";
    case "draft":
      return "Draft setup";
    case "preview":
      return "Preview";
    case "pending":
      return "Pending proof";
  }
}

function protocolLabel(protocol: NetworkCanvasProtocolId): string {
  switch (protocol) {
    case "modbus_tcp":
      return "Modbus TCP";
    case "mqtt":
      return "MQTT";
    case "ethercat":
      return "EtherCAT";
    case "gpio":
      return "GPIO";
    case "loopback":
      return "Loopback";
    case "simulated":
      return "Simulated";
  }
}

function numericValue(value: string): number | undefined {
  const normalized = value.trim().toLowerCase();
  if (normalized === "true") {
    return 1;
  }
  if (normalized === "false") {
    return 0;
  }
  const match = normalized.match(/-?\d+(?:\.\d+)?/);
  if (!match) {
    return undefined;
  }
  const parsed = Number(match[0]);
  return Number.isFinite(parsed) ? parsed : undefined;
}
