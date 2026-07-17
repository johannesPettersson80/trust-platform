import type { RuntimeTarget } from "../runtimeTarget";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import type {
  FleetTopologyConnectorStatus,
  FleetTopologyEndpoint,
  FleetTopologyResponse,
} from "./fleetTopology";
import {
  fetchFleetTopology,
  mergeFleetTopologies,
  offlineTopologyForTarget,
} from "./fleetTopology";

interface ConnectorPointCounts {
  total?: number;
  good?: number;
  degraded?: number;
  unavailable?: number;
}

interface ConnectorStatusReport {
  connector_id?: string;
  protocol?: string;
  kind?: string;
  endpoint?: string;
  state?: string;
  health?: string;
  confidence?: string;
  point_counts?: ConnectorPointCounts;
}

interface ConnectorStatusResponse {
  schema_version?: number;
  connectors?: ConnectorStatusReport[];
}

export type CanonicalConnectorState =
  | "disabled"
  | "configured"
  | "starting"
  | "ready"
  | "degraded"
  | "reconnecting"
  | "stale"
  | "not_ready"
  | "faulted";

export type CanonicalConnectorHealth = "ok" | "degraded" | "faulted" | "unknown";

export function canonicalConnectorState(value: unknown): CanonicalConnectorState {
  switch (stringValue(value)) {
    case "disabled":
    case "configured":
    case "starting":
    case "ready":
    case "degraded":
    case "reconnecting":
    case "stale":
    case "not_ready":
    case "faulted":
      return stringValue(value) as CanonicalConnectorState;
    default:
      throw new Error(`unknown connector state: ${String(value)}`);
  }
}

export function canonicalConnectorHealth(value: unknown): CanonicalConnectorHealth {
  switch (stringValue(value)) {
    case "ok":
    case "degraded":
    case "faulted":
    case "unknown":
      return stringValue(value) as CanonicalConnectorHealth;
    default:
      throw new Error(`unknown connector health: ${String(value)}`);
  }
}

export async function fetchConnectorStatus(
  runtime: RuntimeTarget,
  timeoutMs = 2000
): Promise<ConnectorStatusResponse | undefined> {
  if (runtime.status !== "online_reachable" || !runtime.endpoint) {
    return undefined;
  }
  return await sendRuntimeControlRequest<ConnectorStatusResponse>(
    runtime.endpoint,
    runtime.authToken,
    "connectors.status",
    undefined,
    { timeoutMs }
  );
}

export async function fetchAndMergeFleetTopologiesWithConnectorStatus(
  runtimes: readonly RuntimeTarget[],
  timeoutMs = 2000
): Promise<FleetTopologyResponse | undefined> {
  const responses = await Promise.all(
    runtimes.map(async (runtime) => {
      if (runtime.status !== "online_reachable") {
        return offlineTopologyForTarget(runtime);
      }
      const topology = await fetchFleetTopology(runtime, timeoutMs).catch(() => undefined);
      const connectors = await fetchConnectorStatus(runtime, timeoutMs).catch(() => undefined);
      return mergeConnectorStatusIntoTopology(topology, connectors);
    })
  );
  if (responses.every((response) => response === undefined)) {
    return undefined;
  }
  return mergeFleetTopologies(responses);
}

export function mergeConnectorStatusIntoTopology(
  topology: FleetTopologyResponse | undefined,
  status: ConnectorStatusResponse | undefined
): FleetTopologyResponse | undefined {
  if (!topology || !Array.isArray(status?.connectors) || status.connectors.length === 0) {
    return topology;
  }
  const reports = status.connectors.filter((report) => endpointProtocolForReport(report));
  const used = new Set<number>();
  return {
    ...topology,
    hosts: topology.hosts.map((host) => ({
      ...host,
      runtimes: host.runtimes.map((runtime) => ({
        ...runtime,
        endpoints: runtime.endpoints.map((endpoint) =>
          mergeEndpointConnectorStatus(endpoint, reports, used)
        ),
      })),
      containers: host.containers.map((container) => ({
        ...container,
        runtimes: container.runtimes.map((runtime) => ({
          ...runtime,
          endpoints: runtime.endpoints.map((endpoint) =>
            mergeEndpointConnectorStatus(endpoint, reports, used)
          ),
        })),
      })),
    })),
  };
}

function mergeEndpointConnectorStatus(
  endpoint: FleetTopologyEndpoint,
  reports: readonly ConnectorStatusReport[],
  used: Set<number>
): FleetTopologyEndpoint {
  const index = findConnectorReport(endpoint, reports, used);
  if (index === undefined) {
    return endpoint;
  }
  used.add(index);
  return {
    ...endpoint,
    connector: projectConnectorStatus(reports[index]),
  };
}

function findConnectorReport(
  endpoint: FleetTopologyEndpoint,
  reports: readonly ConnectorStatusReport[],
  used: Set<number>
): number | undefined {
  const exact = reports.findIndex(
    (report, index) =>
      !used.has(index) &&
      endpointProtocolForReport(report) === endpoint.protocol &&
      endpointMatchesReport(endpoint, report)
  );
  if (exact >= 0) {
    return exact;
  }
  const protocolOnly = reports.findIndex(
    (report, index) =>
      !used.has(index) && endpointProtocolForReport(report) === endpoint.protocol
  );
  return protocolOnly >= 0 ? protocolOnly : undefined;
}

function endpointProtocolForReport(report: ConnectorStatusReport): string | undefined {
  const protocol = stringValue(report.protocol);
  const kind = stringValue(report.kind);
  if (!protocol) {
    return undefined;
  }
  if (protocol === "ads") {
    return kind === "supervisory_server" ? "ads_server" : "ads";
  }
  if (protocol === "opcua") {
    return kind === "supervisory_client" ? "opcua_client" : "opcua";
  }
  return protocol;
}

function endpointMatchesReport(endpoint: FleetTopologyEndpoint, report: ConnectorStatusReport): boolean {
  const reportEndpoint = stringValue(report.endpoint);
  if (!reportEndpoint) {
    return false;
  }
  const candidates = [
    stringValue(endpoint.address),
    stringValue(endpoint.params?.endpoint_url),
    stringValue(endpoint.params?.broker),
    stringValue(endpoint.params?.host),
    stringValue(endpoint.params?.listen),
    stringValue(endpoint.params?.interface),
  ].filter((value): value is string => Boolean(value));
  return candidates.some(
    (candidate) =>
      candidate === reportEndpoint ||
      reportEndpoint.includes(candidate) ||
      candidate.includes(reportEndpoint)
  );
}

function projectConnectorStatus(report: ConnectorStatusReport): FleetTopologyConnectorStatus {
  const counts = report.point_counts ?? {};
  return {
    connector_id: stringValue(report.connector_id) || "connector",
    state: canonicalConnectorState(report.state),
    health: canonicalConnectorHealth(report.health),
    confidence: stringValue(report.confidence) || "unavailable",
    point_counts: {
      total: numberValue(counts.total),
      good: numberValue(counts.good),
      degraded: numberValue(counts.degraded),
      unavailable: numberValue(counts.unavailable),
    },
  };
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function numberValue(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}
