import * as vscode from "vscode";

import type { CommApplyResponse, CommSchemaResponse } from "../communication/schemaForm";
import type { CommCapabilitiesResponse } from "../communication/capability";
import { fetchCommSchema } from "../communication/runtimeComm";
import { getControlAuthToken } from "../runtimeAuth";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  resolveRuntimeTarget,
  resolveRuntimeTargetFromSettings,
  type RuntimeTarget,
} from "../runtimeTarget";
import { localSimControl } from "../simControl";
import { getBinaryPath } from "../binary";
import type {
  RuntimeLifecycleSnapshot,
  RuntimeStartFailure,
} from "../runtimeLifecycle";
import { listManagedRuntimes } from "../localRuntime";
import { getSelectedRuntimeId } from "../selectedRuntime";
import { getTrustConfiguration } from "../configuration";
import {
  fetchConnectorStatus,
  fetchAndMergeFleetTopologiesWithConnectorStatus,
  mergeConnectorStatusIntoTopology,
} from "./connectorsStatus";
import { buildCanvasGraph, projectPlantRuntimeId } from "./graphData";
import {
  createPlantSessionRegistry,
  PlantSessionRegistry,
  populatePlantTopology,
  ProjectPlantTopologyService,
  removePlantCandidate,
  savePlantCandidate,
} from "./plant/service";
import {
  fetchFleetTopology,
  mergeFleetTopologies,
  type FleetTopologyResponse,
} from "./fleetTopology";
import {
  buildNetworkCanvasModel,
  type NetworkCanvasFailure,
  type NetworkCanvasProtocolId,
  type NetworkCanvasStage,
} from "./model";
import { offlineCommSchema, offlineCommTopology } from "./offlineComm";
import type { NCGraph } from "./webview/types";

const plantSessions = new WeakMap<vscode.ExtensionContext, PlantSessionRegistry>();

export interface CanvasDataRequest {
  context: vscode.ExtensionContext | undefined;
  stage: NetworkCanvasStage;
  snapshot: RuntimeLifecycleSnapshot;
  hasStructuredTextSession: boolean;
  activeProtocol: NetworkCanvasProtocolId;
  applyResult: CommApplyResponse | undefined;
  searchQuery: string;
  pinnedNodeId: string | undefined;
  quickAddOpen: boolean;
  lastFailure: RuntimeStartFailure | undefined;
  deviceRequested: boolean;
  fleetEndpointLabels: ReadonlyMap<string, string>;
}

export interface CanvasDataSnapshot {
  graph: NCGraph;
  runtime: RuntimeTarget;
  schema: CommSchemaResponse | undefined;
  topology: FleetTopologyResponse | undefined;
  reachable: boolean;
  setupMessage: string | undefined;
}

/** Loads offline configuration and overlays current runtime evidence into one honest canvas view. */
export async function loadCanvasData(request: CanvasDataRequest): Promise<CanvasDataSnapshot> {
  const resource = vscode.workspace.workspaceFolders?.[0]?.uri;
  const projectDir = resource?.fsPath;
  let runtime = await resolveRuntimeTarget(resource);

  // A live debug simulator may provide the control endpoint even when project settings are offline.
  if (runtime.status !== "online_reachable" && request.hasStructuredTextSession) {
    const simulator = localSimControl(projectDir);
    if (simulator) {
      const candidate = await resolveRuntimeTargetFromSettings({
        mode: "online",
        endpoint: simulator.endpoint,
        authToken: simulator.authToken,
        endpointEnabled: true,
        label: "Simulator",
      });
      if (candidate.status === "online_reachable") runtime = candidate;
    }
  }

  let capabilities: CommCapabilitiesResponse | undefined;
  let schema: CommSchemaResponse | undefined;
  let setupMessage: string | undefined;
  let topologyError: string | undefined;
  let baseTopology: FleetTopologyResponse | undefined;
  let offlineTopology: FleetTopologyResponse | undefined;

  if (request.context) {
    schema = await offlineCommSchema(request.context);
    if (projectDir) {
      offlineTopology = await offlineCommTopology(request.context, projectDir);
      baseTopology = offlineTopology;
    }
  }

  if (runtime.status === "online_reachable" && runtime.endpoint) {
    capabilities = await sendRuntimeControlRequest<CommCapabilitiesResponse>(
      runtime.endpoint,
      runtime.authToken,
      "comm.capabilities",
      undefined,
      { timeoutMs: 2000 }
    ).catch(() => undefined);
    try {
      schema = (await fetchCommSchema(runtime)) ?? schema;
    } catch (error) {
      if (!schema) setupMessage = error instanceof Error ? error.message : String(error);
    }
    try {
      const liveTopology = mergeConnectorStatusIntoTopology(
        await fetchFleetTopology(runtime),
        await fetchConnectorStatus(runtime).catch(() => undefined)
      );
      if (liveTopology) {
        baseTopology = offlineTopology
          ? mergeFleetTopologies([liveTopology, offlineTopology])
          : mergeFleetTopologies([liveTopology]);
      }
    } catch (error) {
      topologyError = `Fleet topology unavailable: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  let peerTopology: FleetTopologyResponse | undefined;
  try {
    const peers = (await resolveFleetTargets(runtime, request.fleetEndpointLabels)).filter(
      (target) => target.endpoint && target.endpoint !== runtime.endpoint
    );
    if (peers.length > 0) {
      const peerResult = await fetchAndMergeFleetTopologiesWithConnectorStatus(peers);
      peerTopology = peerResult.topology;
      if (peerResult.errors.length > 0) {
        const peerError = `Peer topology degraded: ${peerResult.errors.join("; ")}`;
        topologyError = topologyError ? `${topologyError}; ${peerError}` : peerError;
      }
    }
  } catch (error) {
    const peerError = `Peer topology unavailable: ${error instanceof Error ? error.message : String(error)}`;
    topologyError = topologyError ? `${topologyError}; ${peerError}` : peerError;
  }

  const displayTopology =
    baseTopology || peerTopology
      ? mergeFleetTopologies([baseTopology, peerTopology])
      : undefined;
  const model = buildNetworkCanvasModel({
    stage: request.stage,
    runtime: request.snapshot.status,
    ioState: request.snapshot.ioState,
    schema,
    capabilities,
    activeProtocol: request.activeProtocol,
    applyResult: request.applyResult,
    searchQuery: request.searchQuery,
    pinnedNodeId: request.pinnedNodeId,
    quickAddOpen: request.quickAddOpen,
    topology: displayTopology,
    topologyError,
    starting: request.snapshot.starting,
    failure: asNetworkFailure(request.lastFailure ?? request.snapshot.failure),
    deviceRequested: request.deviceRequested,
    runtimeSetupMessage: setupMessage,
  });
  const attachedEndpoint =
    request.snapshot.status.runtimeMode === "online" &&
    request.snapshot.status.runtimeState === "connected"
      ? request.snapshot.status.endpoint
      : undefined;
  const managed = request.context ? await listManagedRuntimes(request.context) : [];

  const graph = buildCanvasGraph(
    model,
    displayTopology,
    undefined,
    attachedEndpoint,
    managed,
    getSelectedRuntimeId()
  );
  if (request.context && projectDir) {
    const registry = plantSessionRegistry(request.context);
    const stopped = registry.forProject(projectDir);
    const liveRequest =
      runtime.status === "online_reachable" && runtime.endpoint
        ? async (operation: string, params: Record<string, unknown>) =>
            await sendRuntimeControlRequest(
              runtime.endpoint as string,
              runtime.authToken,
              operation,
              params,
              { timeoutMs: 2_000 }
            )
        : async (operation: string, params: Record<string, unknown>) =>
            await stopped.request(operation, params);
    const owningRuntimeId = projectPlantRuntimeId(graph);
    if (owningRuntimeId) {
      await populatePlantTopology(
        graph,
        owningRuntimeId,
        new ProjectPlantTopologyService(stopped, liveRequest)
      ).catch((error) => {
        // Keep the ordinary canvas usable, but do not make a missing plant
        // capability invisible during extension-host diagnosis.
        console.warn(
          "[truST] Simulated plant configuration is unavailable:",
          error instanceof Error ? error.message : String(error)
        );
      });
    }
  }

  return {
    graph,
    runtime,
    schema,
    topology: baseTopology,
    reachable: runtime.status === "online_reachable",
    setupMessage,
  };
}

function plantSessionRegistry(context: vscode.ExtensionContext): PlantSessionRegistry {
  const existing = plantSessions.get(context);
  if (existing) return existing;
  const registry = createPlantSessionRegistry(
    getBinaryPath(context, "trust-runtime", "runtime.cli.path")
  );
  plantSessions.set(context, registry);
  context.subscriptions.push(registry);
  return registry;
}

/** Saves through the same per-project session already used to populate the canvas. */
export async function saveProjectPlant(
  context: vscode.ExtensionContext,
  projectDir: string,
  candidate: Record<string, unknown>,
  operationId: string,
  expectedRevision?: string
) {
  return await savePlantCandidate(
    plantSessionRegistry(context).forProject(projectDir),
    candidate,
    operationId,
    expectedRevision
  );
}

/** Removes through the same per-project session already used to populate and save the canvas. */
export async function removeProjectPlant(
  context: vscode.ExtensionContext,
  projectDir: string,
  plantId: string,
  operationId: string,
  expectedRevision?: string
) {
  return await removePlantCandidate(
    plantSessionRegistry(context).forProject(projectDir),
    plantId,
    operationId,
    expectedRevision
  );
}

/** Reads plant configuration through the canvas' existing per-project command session. */
export async function readProjectPlants(
  context: vscode.ExtensionContext,
  projectDir: string
) {
  return await plantSessionRegistry(context).forProject(projectDir).request("plant.read", {
    contract: "trust.plant",
    schema_version: 1,
  });
}

async function resolveFleetTargets(
  primary: RuntimeTarget,
  fleetEndpointLabels: ReadonlyMap<string, string>
): Promise<RuntimeTarget[]> {
  const config = getTrustConfiguration(vscode.workspace.workspaceFolders?.[0]?.uri);
  const endpoints = [
    ...new Set(
      (config.get<string[]>("runtime.fleetEndpoints", []) ?? [])
        .map((endpoint) => endpoint.trim())
        .filter((endpoint) => endpoint.length > 0 && endpoint !== primary.endpoint)
    ),
  ];
  const peers = await Promise.all(
    endpoints.map(async (endpoint) =>
      resolveRuntimeTargetFromSettings({
        mode: "online",
        endpoint,
        authToken: await getControlAuthToken(endpoint),
        endpointEnabled: true,
        label: fleetEndpointLabels.get(endpoint),
      }).catch(() => undefined)
    )
  );
  return [primary, ...peers.filter((peer): peer is RuntimeTarget => peer !== undefined)];
}

function asNetworkFailure(failure: RuntimeStartFailure | undefined): NetworkCanvasFailure | undefined {
  if (!failure) return undefined;
  return { kind: failure.kind, message: failure.message, detail: failure.detail };
}
