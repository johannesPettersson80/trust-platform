import type { DiscoverCandidate } from "../offlineComm";
import {
  PLC_RUNTIME_PORTS,
  parseCustomAdsPorts,
  planAdsServicePorts,
} from "../adsServiceProbeModel";

export {
  AUTOMATIC_ADS_SERVICE_PORTS,
  COMMON_ADS_SERVICE_PORTS,
  PLC_RUNTIME_PORTS,
} from "../adsServiceProbeModel";

export interface AdsDiscoveryDraft {
  readonly host: string;
  readonly amsNetId: string;
  readonly customPorts: string;
  readonly advanced: boolean;
}

export const DEFAULT_ADS_DISCOVERY_DRAFT: AdsDiscoveryDraft = {
  host: "",
  amsNetId: "",
  customPorts: "",
  advanced: false,
};

export interface AdsDiscoveryScanSnapshot {
  readonly origin: string;
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
    host: draft.host.trim() || undefined,
    targetAmsNetId: draft.amsNetId.trim() || undefined,
    ports: planAdsServicePorts(customPorts),
  };
}

/**
 * The ordinary ADS action always searches both places a first-time user expects:
 * the AMS router on the discovery computer and that computer's local network.
 * A known address is additive recovery evidence, not a mutually exclusive mode.
 */
export function createAutomaticAdsDiscoveryItems(
  snapshot: Pick<AdsDiscoveryScanSnapshot, "host" | "targetAmsNetId">
): readonly DiscoverRequestItem[] {
  const host = snapshot.host?.trim();
  const isLoopback =
    host === "127.0.0.1" || host?.toLowerCase() === "localhost";
  // Discovery establishes device identity only. Logical ADS services are
  // checked separately through `snapshot.ports`; attaching 851 here would turn
  // a default into unverified candidate data before any ADS service replied.
  const items: DiscoverRequestItem[] = [{ protocol: "ads" }];
  if (host && (!isLoopback || snapshot.targetAmsNetId)) {
    items.push({
      protocol: "ads",
      host,
      targetAmsNetId: snapshot.targetAmsNetId,
    });
  }
  return items;
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

export function adsDiscoveryFields(advanced: boolean): readonly string[] {
  return advanced ? ["host", "ams_net_id", "ads_port"] : [];
}

export function validateAdsDiscoveryHost(host: string): string | undefined {
  const value = host.trim();
  if (!value) {
    return "Enter the known ADS Host or IP.";
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
      draft.host.trim().length > 0
        ? validateAdsDiscoveryHost(draft.host)
        : draft.amsNetId.trim().length > 0
          ? "Enter a known Host or IP when supplying an AMS Net ID."
          : undefined,
    // Persisted Advanced values remain active while collapsed, so validate them too.
    amsNetIdError: validateAdsAmsNetId(draft.amsNetId),
    customPortError: parseCustomAdsPorts(draft.customPorts).error,
  };
}

export function autoSelectAdsServicePort(
  availablePorts: readonly number[]
): number | undefined {
  const unique = [...new Set(availablePorts)];
  return unique.length === 1 ? unique[0] : undefined;
}

export function adsServicePresentation(port: number): {
  readonly primary: string;
  readonly secondary: string;
} {
  const commonIndex = PLC_RUNTIME_PORTS.indexOf(
    port as (typeof PLC_RUNTIME_PORTS)[number]
  );
  return {
    primary: `ADS ${port}`,
    secondary:
      commonIndex >= 0
        ? `PLC runtime ${commonIndex + 1}`
        : port === 301 || port === 501
          ? "Common ADS service"
            : "Custom ADS service",
  };
}

export interface DiscoverRequest {
  readonly origin: string;
  readonly originEndpoint?: string;
  readonly items: readonly DiscoverRequestItem[];
}

export function discoveryOriginForMode(
  mode: "ads" | "selected",
  hardwareOrigin: string
): string {
  return mode === "ads" ? "this_host" : hardwareOrigin;
}

export interface DiscoverProgressRow {
  readonly protocol: string;
  readonly label: string;
  readonly status: "scanning" | "done" | "failed";
  readonly count?: number;
}

export function discoveryProgressCopy(row: DiscoverProgressRow): string {
  if (row.status === "scanning") {
    if (row.protocol === "ads") {
      return "Searching this computer and local network…";
    }
    return `${row.label} … scanning`;
  }
  if (row.status === "failed") {
    return `${row.label} … failed`;
  }
  if (row.protocol === "ads") {
    const count = row.count ?? 0;
    return count === 0
      ? `${row.label} … no ADS devices found`
      : `${row.label} … ${count} ADS device${count === 1 ? "" : "s"} found`;
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
  if (snapshot.host) {
    const target = snapshot.host ? ` at ${snapshot.host}` : " at the known address";
    return snapshot.targetAmsNetId
      ? `No ADS device answered${target}. Check the address and AMS Net ID, make sure the device is running, and confirm your firewall allows truST on this network.`
      : `No ADS device answered${target}. Check the address, make sure the device is running, and confirm your firewall allows truST on this network. If you know its AMS Net ID, add it in Advanced.`;
  }
  return "No ADS devices answered on this computer or the local network. Make sure the device is running and that your firewall allows truST on this network, then try again. If you know its address, use Advanced.";
}

export function applyAdsEmptyRecovery(
  draft: AdsDiscoveryDraft,
  _snapshot: AdsDiscoveryScanSnapshot
): AdsDiscoveryDraft {
  return { ...draft, advanced: true };
}

export type AdsRecoveryFocusRole =
  | "ads-host"
  | "ads-ams-net-id"
  | "ads-custom-ports";

export function adsEmptyRecoveryFocusRole(
  snapshot: AdsDiscoveryScanSnapshot,
  errors: {
    readonly hostError?: string;
    readonly amsNetIdError?: string;
    readonly customPortError?: string;
  }
): AdsRecoveryFocusRole {
  if (!snapshot.host) {
    return "ads-host";
  }
  if (!snapshot.targetAmsNetId) {
    return "ads-ams-net-id";
  }
  if (errors.hostError) {
    return "ads-host";
  }
  if (errors.amsNetIdError) {
    return "ads-ams-net-id";
  }
  if (errors.customPortError) {
    return "ads-custom-ports";
  }
  return "ads-host";
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
