import type * as vscode from "vscode";

import type { CommCapabilitiesResponse } from "../communication/capability";
import type { CommSchemaResponse } from "../communication/schemaForm";
import { fetchCommSchema } from "../communication/runtimeComm";
import { listManagedRuntimes } from "../localRuntime";
import type { ManagedRuntime } from "../localRuntimeModel";
import type { RuntimeTarget } from "../runtimeTarget";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  fetchConnectorStatus,
  mergeConnectorStatusIntoTopology,
  type FleetTopologyConnectorMergeResult,
} from "./connectorsStatus";
import {
  fetchFleetTopology,
  mergeFleetTopologies,
  type FleetTopologyResponse,
} from "./fleetTopology";
import { offlineCommSchema, offlineCommTopology } from "./offlineComm";

export interface NetworkCanvasRefreshData {
  readonly schema?: CommSchemaResponse;
  readonly capabilities?: CommCapabilitiesResponse;
  readonly localTopology?: FleetTopologyResponse;
  readonly displayTopology?: FleetTopologyResponse;
  readonly topologyError?: string;
  readonly runtimeSetupMessage?: string;
  readonly managed: ManagedRuntime[];
}

interface LiveRefreshData {
  readonly schema?: CommSchemaResponse;
  readonly capabilities?: CommCapabilitiesResponse;
  readonly topology?: FleetTopologyResponse;
  readonly schemaError?: string;
  readonly topologyError?: string;
}

export async function loadNetworkCanvasRefreshData(options: {
  readonly context?: vscode.ExtensionContext;
  readonly projectDir?: string;
  readonly runtime: RuntimeTarget;
  readonly loadPeerTopology: () => Promise<FleetTopologyConnectorMergeResult>;
}): Promise<NetworkCanvasRefreshData> {
  const offlinePromise = loadOfflineData(options.context, options.projectDir);
  const livePromise = loadLiveData(options.runtime);
  const managedPromise = options.context
    ? listManagedRuntimes(options.context)
    : Promise.resolve([]);
  const peerPromise = options.loadPeerTopology().catch((error) => ({
    topology: undefined,
    errors: [`Peer topology unavailable: ${errorMessage(error)}`],
  }));

  const [offline, live, managed, peer] = await Promise.all([
    offlinePromise,
    livePromise,
    managedPromise,
    peerPromise,
  ]);

  const schema = live.schema ?? offline.schema;
  const localTopology = live.topology || offline.topology
    ? mergeFleetTopologies([live.topology, offline.topology])
    : undefined;
  const displayTopology = localTopology || peer.topology
    ? mergeFleetTopologies([localTopology, peer.topology])
    : undefined;
  const peerError = peer.errors.length > 0
    ? `Peer topology degraded: ${peer.errors.join("; ")}`
    : undefined;

  return {
    schema,
    capabilities: live.capabilities,
    localTopology,
    displayTopology,
    topologyError: [live.topologyError, peerError].filter(Boolean).join("; ") || undefined,
    runtimeSetupMessage: schema ? undefined : live.schemaError,
    managed,
  };
}

async function loadOfflineData(
  context: vscode.ExtensionContext | undefined,
  projectDir: string | undefined,
): Promise<{
  readonly schema?: CommSchemaResponse;
  readonly topology?: FleetTopologyResponse;
}> {
  if (!context) {
    return {};
  }
  const [schema, topology] = await Promise.all([
    offlineCommSchema(context),
    projectDir
      ? offlineCommTopology(context, projectDir)
      : Promise.resolve(undefined),
  ]);
  return { schema, topology };
}

async function loadLiveData(runtime: RuntimeTarget): Promise<LiveRefreshData> {
  if (runtime.status !== "online_reachable" || !runtime.endpoint) {
    return {};
  }
  const [capabilities, schema, topology] = await Promise.allSettled([
    sendRuntimeControlRequest<CommCapabilitiesResponse>(
      runtime.endpoint,
      runtime.authToken,
      "comm.capabilities",
      undefined,
      { timeoutMs: 2000 },
    ),
    fetchCommSchema(runtime),
    Promise.all([
      fetchFleetTopology(runtime),
      fetchConnectorStatus(runtime).catch(() => undefined),
    ]).then(([fleet, connectors]) =>
      mergeConnectorStatusIntoTopology(fleet, connectors)
    ),
  ]);

  return {
    capabilities:
      capabilities.status === "fulfilled" ? capabilities.value : undefined,
    schema: schema.status === "fulfilled" ? schema.value : undefined,
    topology: topology.status === "fulfilled" ? topology.value : undefined,
    schemaError:
      schema.status === "rejected" ? errorMessage(schema.reason) : undefined,
    topologyError:
      topology.status === "rejected"
        ? `Fleet topology unavailable: ${errorMessage(topology.reason)}`
        : undefined,
  };
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
