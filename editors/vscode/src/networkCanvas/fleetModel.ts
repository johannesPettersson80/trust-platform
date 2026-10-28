// The graph that the canvas draws, as `trust-lsp.devicesTopology` answers it
// (`crates/trust-devices/src/canvas`; specification 25 §5.5, §5.6 and §5.15): computer headings,
// health roll-ups, the drawn simulator and its connections, the managed runtimes, the session's
// marks, product names, the search's dimming, the browse targets, Identify, the ADS client
// summaries, the faults, the banner and the summary are all the server's. The canvas maps the
// answer into its graph without deciding anything.

import type { FleetTopologyConnectorStatus, FleetTopologySlave } from "./fleetTopology";

export interface CanvasFleetEndpoint {
  readonly id: string;
  readonly kind: string;
  readonly protocol: string;
  readonly name: string;
  readonly role?: string;
  readonly health: string;
  readonly detail: string;
  readonly owned: boolean;
  readonly live?: { value?: unknown; last_seen_ms?: number; rtt_ms?: number };
  readonly connector?: FleetTopologyConnectorStatus;
  readonly params?: Record<string, unknown>;
  readonly children?: readonly FleetTopologySlave[];
  readonly dimmed: boolean;
  /** The target the browse pane opens with, for an ADS or OPC UA client connection. */
  readonly browseTarget?: Record<string, unknown>;
  /** Whether a filter that hides it reports it. */
  readonly attention: boolean;
  /** The ADS client's summary. */
  readonly adsSummary?: CanvasAdsClientSummary;
}

/** The ADS client's summary, as the server words it. */
export interface CanvasAdsClientSummary {
  readonly devices: ReadonlyArray<{
    readonly address: string;
    readonly amsNetId: string;
    readonly ports: ReadonlyArray<{ readonly port: number; readonly tagCount: number }>;
  }>;
  readonly status: string;
  readonly statusKind: "ok" | "neutral" | "error";
  readonly enabled: boolean;
  readonly configPath: string;
  readonly updateIntervalMs: number;
}

/** Whether an action can run now, and why not. */
export interface CanvasActionState {
  readonly enabled: boolean;
  readonly reason?: string;
}

export interface CanvasFleetRuntime {
  readonly id: string;
  readonly name: string;
  readonly mode: string;
  readonly health: string;
  readonly detail: string;
  readonly endpointCount: number;
  /** The control endpoint the runtime is reached through; none for the extension's own simulator. */
  readonly controlEndpoint?: string;
  readonly endpoints: readonly CanvasFleetEndpoint[];
  readonly managed: boolean;
  readonly managedName?: string;
  readonly lifecycleState?: string;
  readonly attached: boolean;
  readonly runTarget: boolean;
  readonly identify: CanvasActionState;
  /** The controls of its node, the primary one first (specification 25 §5.5). */
  readonly controls: readonly CanvasRuntimeControl[];
  /** Whether a search can start from it, and why not (specification 25 §2). */
  readonly discovery: { readonly ready: boolean; readonly reason?: string };
}

/** One control of a runtime node, as the language server gives it. */
export interface CanvasRuntimeControl {
  readonly action: string;
  readonly label: string;
  readonly kind: "primary" | "secondary";
  readonly enabled: boolean;
}

export interface CanvasFleetContainer {
  readonly id: string;
  readonly name: string;
  readonly image: string;
  readonly status: string;
  readonly runtimes: readonly CanvasFleetRuntime[];
}

export interface CanvasFleetHost {
  readonly id: string;
  readonly hostname: string;
  readonly label: string;
  readonly health: string;
  readonly runtimeCount: number;
  readonly endpointCount: number;
  readonly containers: readonly CanvasFleetContainer[];
  readonly runtimes: readonly CanvasFleetRuntime[];
}

export interface CanvasFleetLink {
  readonly id: string;
  readonly from: string;
  readonly to: string;
  readonly protocol: string;
  readonly role: string;
  readonly status: string;
  readonly detail?: string;
  readonly secure: boolean;
}

export interface CanvasFleetExternal {
  readonly id: string;
  readonly name: string;
  readonly kind: string;
}

export interface CanvasFleetFault {
  readonly id: string;
  readonly label: string;
  readonly targetNodeId: string;
  readonly severity: "warning" | "error";
  /** The fault of the add form's refused result, which the canvas hides while the form is edited. */
  readonly form?: boolean;
}

export interface CanvasBanner {
  readonly kind: "info" | "error";
  readonly text: string;
  readonly actions: ReadonlyArray<{ readonly label: string; readonly action: string }>;
}

export interface CanvasFleet {
  readonly hosts: readonly CanvasFleetHost[];
  readonly links: readonly CanvasFleetLink[];
  /** The shared systems, then the external systems, as the nodes the canvas draws for them. */
  readonly external: readonly CanvasFleetExternal[];
  readonly externalCount: number;
  readonly sharedCount: number;
  readonly summary: string;
  readonly faults: readonly CanvasFleetFault[];
  readonly banner?: CanvasBanner;
}
