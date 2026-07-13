import type { CommCapabilitiesResponse } from "../communication/capability";
import type {
  CommApplyResponse,
  CommSchemaResponse,
} from "../communication/schemaForm";
import {
  effectiveLifecycleEntryFailure,
  type LifecycleAction,
} from "../lifecycleEntryFailure";
import type {
  RuntimeLifecycleSnapshot,
  RuntimeStartFailure,
} from "../runtimeLifecycle";
import type { RuntimeLifecycleTarget } from "../runtimeLifecycleModel";
import type { ManagedRuntime } from "../localRuntimeModel";
import type { FleetTopologyResponse } from "./fleetTopology";
import { buildCanvasGraph } from "./graphData";
import type {
  BuildNetworkCanvasModelInput,
  NetworkCanvasFailure,
  NetworkCanvasProtocolId,
  NetworkCanvasStage,
} from "./model";
import { buildNetworkCanvasModel } from "./model";
import { immediateSimulatorLifecycleProjection } from "./lifecycleRefreshPolicy";
import { projectCanvasLifecycleAuthority } from "./lifecycleAuthorityProjection";

export interface NetworkCanvasLifecycleState {
  readonly lastFailure?: RuntimeStartFailure;
  readonly lastFailureAction?: LifecycleAction;
  readonly deviceRequested: boolean;
}

export interface NetworkCanvasSnapshotOptions {
  readonly schema?: CommSchemaResponse;
  readonly capabilities?: CommCapabilitiesResponse;
  readonly activeProtocol?: NetworkCanvasProtocolId;
  readonly applyResult?: CommApplyResponse;
  readonly searchQuery?: string;
  readonly pinnedNodeId?: string;
  readonly quickAddOpen?: boolean;
  readonly topology?: FleetTopologyResponse;
  readonly topologyError?: string;
  readonly runtimeSetupMessage?: string;
  /** Exact transition/accepted owner used to scope local Simulator state. */
  readonly authorityTarget?: RuntimeLifecycleTarget;
}

export interface ImmediateSimulatorLifecycleGraphInput {
  readonly phase: "stopped" | "starting" | "running" | "connected";
  readonly stage: NetworkCanvasStage;
  readonly lastFailure?: RuntimeStartFailure;
  readonly lastFailureAction?: LifecycleAction;
  readonly localFailure?: RuntimeStartFailure;
  readonly schema?: CommSchemaResponse;
  readonly activeProtocol?: NetworkCanvasProtocolId;
  readonly applyResult?: CommApplyResponse;
  readonly searchQuery?: string;
  readonly pinnedNodeId?: string;
  readonly quickAddOpen?: boolean;
  readonly topology?: FleetTopologyResponse;
  readonly managedRuntimes: ReadonlyArray<ManagedRuntime>;
  readonly selectedRuntimeId: string;
  readonly deviceRequested: boolean;
  readonly authorityTarget?: RuntimeLifecycleTarget;
}

export function buildImmediateSimulatorLifecycleGraph(
  input: ImmediateSimulatorLifecycleGraphInput
): ReturnType<typeof buildCanvasGraph> | undefined {
  const simulatorOwned = input.authorityTarget?.kind === "simulator";
  const projection = simulatorOwned
    ? immediateSimulatorLifecycleProjection(input.phase)
    : undefined;
  if (!projection && !input.authorityTarget) {
    return undefined;
  }
  const immediateFailure =
    input.lastFailureAction === "connect" ||
    input.lastFailureAction === "disconnect"
      ? input.localFailure
      : input.lastFailure ?? input.localFailure;
  const model = buildNetworkCanvasModel({
    stage: input.stage,
    runtime: projection?.running
      ? {
          running: true,
          runtimeState: "running",
          runtimeMode: "simulate",
        }
      : undefined,
    starting: projection?.starting ?? false,
    failure: asNetworkFailure(immediateFailure),
    ioState: undefined,
    schema: input.schema,
    activeProtocol: input.activeProtocol,
    applyResult: input.applyResult,
    searchQuery: input.searchQuery,
    pinnedNodeId: input.pinnedNodeId,
    quickAddOpen: input.quickAddOpen,
    topology: input.topology,
    deviceRequested: input.deviceRequested,
  });
  const graph = buildCanvasGraph(
    model,
    input.topology,
    undefined,
    undefined,
    input.managedRuntimes,
    input.selectedRuntimeId
  );
  return projectCanvasLifecycleAuthority(graph, {
    phase: input.phase,
    target: input.authorityTarget,
  });
}

export function modelInputForSnapshot(
  stage: NetworkCanvasStage,
  snapshot: RuntimeLifecycleSnapshot | undefined,
  state: NetworkCanvasLifecycleState,
  options: NetworkCanvasSnapshotOptions = {}
): BuildNetworkCanvasModelInput {
  const localSimulatorFailure =
    state.lastFailureAction === "connect" ||
    state.lastFailureAction === "disconnect"
      ? undefined
      : state.lastFailure;
  const lifecycleSimulatorFailure =
    snapshot?.failureScope?.kind === "remote"
      ? undefined
      : snapshot?.failure;
  return {
    stage,
    runtime:
      options.authorityTarget && options.authorityTarget.kind !== "simulator"
        ? undefined
        : snapshot?.status,
    ioState: snapshot?.ioState,
    schema: options.schema,
    capabilities: options.capabilities,
    activeProtocol: options.activeProtocol,
    applyResult: options.applyResult,
    searchQuery: options.searchQuery,
    pinnedNodeId: options.pinnedNodeId,
    quickAddOpen: options.quickAddOpen,
    topology: options.topology,
    topologyError: options.topologyError,
    starting:
      snapshot?.starting && snapshot.transitionTarget?.kind === "simulator",
    failure: asNetworkFailure(
      snapshot
        ? effectiveLifecycleEntryFailure(
            localSimulatorFailure,
            lifecycleSimulatorFailure,
            state.lastFailureAction,
            snapshot.starting
              ? "starting"
              : snapshot.status.runtimeState
          )
        : localSimulatorFailure
    ),
    deviceRequested: state.deviceRequested,
    runtimeSetupMessage: options.runtimeSetupMessage,
  };
}

export function asNetworkFailure(
  failure: RuntimeStartFailure | undefined
): NetworkCanvasFailure | undefined {
  if (!failure) {
    return undefined;
  }
  return {
    kind: failure.kind,
    message: failure.message,
    detail: failure.detail,
  };
}
