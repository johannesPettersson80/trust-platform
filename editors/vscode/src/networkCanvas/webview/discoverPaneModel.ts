import type { DiscoverCandidate } from "../offlineComm";
import {
  PLC_RUNTIME_PORTS,
  parseCustomAdsPorts,
  planAdsServicePorts,
} from "../adsServiceProbeModel";

export {
  AUTOMATIC_TWINCAT_SERVICE_PORTS,
  COMMON_TWINCAT_SERVICE_PORTS,
  PLC_RUNTIME_PORTS,
} from "../adsServiceProbeModel";

export type AdsDiscoveryLocationId =
  | "same_computer"
  | "local_network"
  | "known_address";

export const ADS_DISCOVERY_LOCATIONS = [
  {
    id: "same_computer",
    label: "On the discovery computer",
    recommended: false,
  },
  {
    id: "local_network",
    label: "On the discovery computer's network",
    recommended: true,
  },
  {
    id: "known_address",
    label: "At known address",
    recommended: false,
  },
] as const;

export interface AdsDiscoveryDraft {
  readonly location: AdsDiscoveryLocationId;
  readonly host: string;
  readonly amsNetId: string;
  readonly customPorts: string;
  readonly advanced: boolean;
}

export const DEFAULT_ADS_DISCOVERY_DRAFT: AdsDiscoveryDraft = {
  location: "local_network",
  host: "",
  amsNetId: "",
  customPorts: "",
  advanced: false,
};

export interface AdsDiscoveryScanSnapshot {
  readonly origin: string;
  readonly location: AdsDiscoveryLocationId;
  readonly host?: string;
  readonly targetAmsNetId?: string;
  readonly ports: readonly number[];
}

export function createAdsDiscoveryScanSnapshot(
  origin: string,
  draft: AdsDiscoveryDraft
): AdsDiscoveryScanSnapshot {
  const customPorts = parseCustomAdsPorts(draft.customPorts).ports;
  return {
    origin,
    location: draft.location,
    host:
      draft.location === "same_computer"
        ? "127.0.0.1"
        : draft.location === "known_address"
          ? draft.host.trim() || undefined
          : undefined,
    targetAmsNetId:
      draft.location === "known_address"
        ? draft.amsNetId.trim() || undefined
        : undefined,
    ports: planAdsServicePorts(customPorts),
  };
}

export interface DiscoverOrigin {
  readonly id: string;
  readonly label: string;
  readonly runtimeDiscoveryReady?: boolean;
  readonly runtimeDiscoveryDisabledReason?: string;
  readonly controlEndpoint?: string;
}

export interface DiscoverRequestItem {
  readonly protocol: string;
  readonly cidr?: string;
  readonly host?: string;
  readonly targetAmsNetId?: string;
  readonly amsPort?: number;
}

export function adsDiscoveryFields(
  location: AdsDiscoveryLocationId,
  advanced: boolean
): readonly string[] {
  const fields: string[] = location === "known_address" ? ["host"] : [];
  if (advanced) {
    if (location === "known_address") {
      fields.push("ams_net_id");
    }
    fields.push("ads_port");
  }
  return fields;
}

export function validateAdsDiscoveryHost(host: string): string | undefined {
  const value = host.trim();
  if (!value) {
    return "Enter the TwinCAT Host or IP.";
  }
  if (
    value.includes("://") ||
    /^\[[^\]]+\]:\d+$/.test(value) ||
    (value.match(/:/g) ?? []).length === 1
  ) {
    return "Enter a Host or IP without a port. PLC runtime ports are selected separately.";
  }
  return undefined;
}

export function validateAdsAmsNetId(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  const parts = trimmed.split(".");
  if (
    parts.length !== 6 ||
    parts.some(
      (part) =>
        !/^\d+$/.test(part) || Number(part) < 0 || Number(part) > 255
    )
  ) {
    return "AMS Net ID must contain six decimal numbers from 0 to 255.";
  }
  return undefined;
}

export function validateAdsDiscoveryDraft(draft: AdsDiscoveryDraft): {
  readonly hostError?: string;
  readonly amsNetIdError?: string;
  readonly customPortError?: string;
} {
  return {
    hostError:
      draft.location === "known_address"
        ? validateAdsDiscoveryHost(draft.host)
        : undefined,
    // Persisted Advanced values remain active while collapsed, so validate them too.
    amsNetIdError:
      draft.location === "known_address"
        ? validateAdsAmsNetId(draft.amsNetId)
        : undefined,
    customPortError: parseCustomAdsPorts(draft.customPorts).error,
  };
}

export function autoSelectTwinCatServicePort(
  availablePorts: readonly number[]
): number | undefined {
  const unique = [...new Set(availablePorts)];
  return unique.length === 1 ? unique[0] : undefined;
}

export function twinCatServicePresentation(port: number): {
  readonly primary: string;
  readonly secondary: string;
} {
  const commonIndex = PLC_RUNTIME_PORTS.indexOf(
    port as (typeof PLC_RUNTIME_PORTS)[number]
  );
  return {
    primary:
      commonIndex >= 0
        ? `PLC runtime ${commonIndex + 1}`
        : port === 301
          ? "Additional task 1"
          : port === 501
            ? "NC SAF service"
            : "Custom service",
    secondary: `ADS ${port}`,
  };
}

export interface DiscoverRequest {
  readonly origin: string;
  readonly originEndpoint?: string;
  readonly items: readonly DiscoverRequestItem[];
}

export interface DiscoverProgressRow {
  readonly protocol: string;
  readonly label: string;
  readonly status: "scanning" | "done" | "failed";
  readonly count?: number;
}

export function discoveryProgressCopy(row: DiscoverProgressRow): string {
  if (row.status === "scanning") {
    return `${row.label} … scanning`;
  }
  if (row.status === "failed") {
    return `${row.label} … failed`;
  }
  if (row.protocol === "ads") {
    const count = row.count ?? 0;
    return count === 0
      ? "No TwinCAT computer found"
      : `${row.label} … ${count} computer${count === 1 ? "" : "s"} found`;
  }
  return `${row.label} … ${row.count ?? 0} found`;
}

export function shouldShowScanSelected(
  selectedProtocols: readonly string[]
): boolean {
  return selectedProtocols.some((protocol) => protocol !== "ads");
}

export function adsServiceProbeResultsNeedRecheck(
  previousPortPlanKey: string | undefined,
  currentPortPlanKey: string,
  validationError?: string
): boolean {
  return Boolean(
    previousPortPlanKey &&
      (previousPortPlanKey !== currentPortPlanKey || validationError)
  );
}

export function adsEmptyIdentityCopy(
  snapshot: AdsDiscoveryScanSnapshot
): string {
  if (snapshot.location === "known_address") {
    const target = snapshot.host ? ` at ${snapshot.host}` : " at the known address";
    return snapshot.targetAmsNetId
      ? `No TwinCAT identity answered${target}. Check the Host, AMS Net ID, route, and Windows firewall, then try again.`
      : `No TwinCAT identity answered${target}. UDP Identify may be blocked; open Advanced and enter the AMS Net ID to use a manual identity fallback.`;
  }
  if (snapshot.location === "same_computer") {
    return "No TwinCAT identity answered on the discovery computer. Confirm TwinCAT is running, or use its known Host and AMS Net ID if UDP Identify is blocked.";
  }
  return "No TwinCAT identity answered on the local network. Confirm TwinCAT is running and allowed through Windows Firewall, or use a known Host and AMS Net ID.";
}

export function applyAdsEmptyRecovery(
  draft: AdsDiscoveryDraft,
  snapshot: AdsDiscoveryScanSnapshot
): AdsDiscoveryDraft {
  return snapshot.location === "known_address"
    ? { ...draft, advanced: true }
    : { ...draft, location: "known_address" };
}

export interface DiscoverCanvasNode {
  readonly id: string;
  readonly type?: string;
  readonly data: Record<string, unknown>;
}

export interface DiscoverSchema {
  readonly protocols: readonly {
    readonly id: string;
    readonly actions: readonly string[];
  }[];
}

export interface DeviceDraft {
  readonly runtimeId: string;
  readonly runtimeName: string;
  readonly protocol: string;
  readonly prefillParams?: Record<string, unknown>;
}

export interface DiscoveredRuntimeHost {
  readonly endpoint: string;
  readonly label: string;
}

export function buildDiscoverOrigins(
  nodes: readonly DiscoverCanvasNode[]
): DiscoverOrigin[] {
  const runtimes = nodes
    .filter((node) => node.type === "runtime")
    .map((node): DiscoverOrigin => {
      const label = String(node.data.label ?? node.id);
      const health = String(node.data.health ?? "");
      const runtimeDiscoveryReady =
        node.data.attached === true ||
        health === "connected" ||
        health === "running" ||
        health === "online";
      return {
        id: node.id,
        label,
        controlEndpoint:
          typeof node.data.controlEndpoint === "string"
            ? node.data.controlEndpoint
            : undefined,
        runtimeDiscoveryReady,
        runtimeDiscoveryDisabledReason: runtimeDiscoveryReady
          ? undefined
          : `Start or connect ${label} before scanning from it.`,
      };
    });
  return [
    {
      id: "this_host",
      label: "This computer",
      runtimeDiscoveryReady: false,
      runtimeDiscoveryDisabledReason:
        "Choose a running runtime for EtherCAT or GPIO scans.",
    },
    ...runtimes,
  ];
}

export function discoverableProtocols(
  schema: DiscoverSchema | undefined
): ReadonlySet<string> {
  return new Set(
    (schema?.protocols ?? [])
      .filter((protocol) => protocol.actions.includes("discover"))
      .map((protocol) => protocol.id)
  );
}

export function shouldShowDiscoveryUnavailable(
  discoverableRowCount: number,
  scanning: boolean,
  progressCount: number,
  resultCount: number,
  error?: string
): boolean {
  return (
    discoverableRowCount === 0 &&
    !scanning &&
    progressCount === 0 &&
    resultCount === 0 &&
    !error
  );
}

export function draftForDiscoveredCandidate(
  candidate: DiscoverCandidate,
  nodes: readonly DiscoverCanvasNode[]
): DeviceDraft {
  const runtime = candidate.originRuntimeId
    ? nodes.find(
        (node) =>
          node.type === "runtime" && node.id === candidate.originRuntimeId
      )
    : nodes.find((node) => node.type === "runtime");
  return {
    runtimeId: runtime?.id ?? "",
    runtimeName: String(runtime?.data.label ?? "runtime"),
    protocol: candidate.protocol,
    prefillParams: candidate.params,
  };
}

export function hostForDiscoveredRuntime(
  candidate: DiscoverCandidate
): DiscoveredRuntimeHost | undefined {
  const endpoint =
    typeof candidate.params.control_endpoint === "string"
      ? candidate.params.control_endpoint
      : "";
  if (!endpoint) {
    return undefined;
  }
  const label =
    typeof candidate.label === "string" && candidate.label.trim().length > 0
      ? candidate.label.trim()
      : typeof candidate.params.name === "string"
        ? candidate.params.name.trim()
        : "";
  return { endpoint, label };
}
