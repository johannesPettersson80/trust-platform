import type { DiscoverCandidate } from "../offlineComm";

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
}

export interface DiscoverRequest {
  readonly origin: string;
  readonly originEndpoint?: string;
  readonly items: readonly DiscoverRequestItem[];
}

export interface DiscoverProgressRow {
  readonly protocol: string;
  readonly label: string;
  readonly status: "scanning" | "done";
  readonly count?: number;
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
