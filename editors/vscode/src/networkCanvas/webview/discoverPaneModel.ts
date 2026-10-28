import type { DiscoveredAdsFacts } from "../canvasCommands";
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

// Whether a search can start from a runtime is the language server's (`discovery` of the node).
export function buildDiscoverOrigins(
  nodes: readonly DiscoverCanvasNode[]
): DiscoverOrigin[] {
  const runtimes = nodes
    .filter((node) => node.type === "runtime")
    .map((node): DiscoverOrigin => {
      const discovery = node.data.discovery as
        | { readonly ready: boolean; readonly reason?: string }
        | undefined;
      return {
        id: node.id,
        label: String(node.data.label ?? node.id),
        controlEndpoint:
          typeof node.data.controlEndpoint === "string"
            ? node.data.controlEndpoint
            : undefined,
        runtimeDiscoveryReady: discovery?.ready === true,
        runtimeDiscoveryDisabledReason: discovery?.ready ? undefined : discovery?.reason,
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

// The runtime a found device is added to, and the host of a found runtime, are the language
// server's (`draft_runtime_id`, `host` of the result's facts).
export function draftForDiscoveredCandidate(
  candidate: DiscoverCandidate,
  facts: DiscoveredAdsFacts | undefined,
  nodes: readonly DiscoverCanvasNode[]
): DeviceDraft {
  const runtimeId = facts?.draft_runtime_id ?? "";
  const runtime = nodes.find((node) => node.type === "runtime" && node.id === runtimeId);
  return {
    runtimeId,
    runtimeName: String(runtime?.data.label ?? "runtime"),
    protocol: candidate.protocol,
    prefillParams: candidate.params,
  };
}

export function hostForDiscoveredRuntime(
  facts: DiscoveredAdsFacts | undefined
): DiscoveredRuntimeHost | undefined {
  return facts?.host;
}
