import type { RoutePlan, SymbolNode } from "../offlineComm";
import {
  adsConnectionsForTarget,
  adsTagSelectionsFromConnections,
  normalizeAdsTagSelections,
  type AdsTagBatchImportResult,
} from "../adsTagBatch";
import {
  adsDiscoveryPorts,
  adsPortBrowseEvidence,
} from "../adsDiscoveryPorts";
import { adsTargetPort, withAdsTargetPort } from "./adsTargetPort";
import { browseAction } from "./browseActions";
import type { BrowseErrorView } from "./browseErrorModel";

export interface BrowsePanelState {
  readonly label: string;
  readonly protocol: string;
  readonly target: Record<string, unknown>;
  readonly title: string;
  readonly actionLabel: string;
  readonly mode: "tags" | "expose";
}

export interface BrowseSymbolsRequest {
  readonly protocol: string;
  readonly target: Record<string, unknown>;
  readonly kind: "symbols" | "channels" | "nodes";
}

export interface BrowseMessageIdentity {
  readonly browseSessionId: string;
  readonly browseRequestId: number;
}

export function isCurrentBrowseMessage(
  message: unknown,
  sessionId: string,
  requestId: number
): message is Record<string, unknown> & BrowseMessageIdentity {
  return (
    isRecord(message) &&
    message.browseSessionId === sessionId &&
    message.browseRequestId === requestId
  );
}

export interface BrowseSessionState {
  readonly panel?: BrowsePanelState;
  readonly tree?: SymbolNode[];
  readonly routeMissing: boolean;
  readonly routePlan?: RoutePlan;
  readonly error?: BrowseErrorView;
  readonly loading: boolean;
  readonly adsImportLoading: boolean;
  readonly adsImportResult?: AdsTagBatchImportResult;
}

export type BrowseSessionAction =
  | {
      readonly type: "open";
      readonly panel: BrowsePanelState;
      readonly loading: boolean;
    }
  | { readonly type: "request"; readonly target: Record<string, unknown> }
  | {
      readonly type: "result";
      readonly tree: SymbolNode[];
      readonly routeMissing: boolean;
      readonly routePlan?: RoutePlan;
      readonly error?: BrowseErrorView;
    }
  | { readonly type: "adsImportStarted" }
  | {
      readonly type: "adsImportResult";
      readonly result: AdsTagBatchImportResult;
    }
  | { readonly type: "close" }
  | { readonly type: "reset" };

export interface BrowseOpenPlan {
  readonly panel: BrowsePanelState;
  readonly loading: boolean;
  readonly request?: BrowseSymbolsRequest;
}

export const EMPTY_BROWSE_SESSION: BrowseSessionState = {
  routeMissing: false,
  loading: false,
  adsImportLoading: false,
};

export function reduceBrowseSessionState(
  state: BrowseSessionState,
  action: BrowseSessionAction
): BrowseSessionState {
  switch (action.type) {
    case "open":
      return {
        panel: action.panel,
        routeMissing: false,
        loading: action.loading,
        adsImportLoading: false,
      };
    case "request":
      if (!state.panel) {
        return state;
      }
      return {
        ...state,
        panel: { ...state.panel, target: action.target },
        tree: undefined,
        routeMissing: false,
        routePlan: undefined,
        error: undefined,
        loading: true,
      };
    case "result":
      return {
        ...state,
        tree: action.tree,
        routeMissing: action.routeMissing,
        routePlan: action.routePlan,
        error: action.error,
        loading: false,
      };
    case "adsImportStarted":
      return state.panel
        ? { ...state, adsImportLoading: true, adsImportResult: undefined }
        : state;
    case "adsImportResult":
      return state.panel
        ? {
            ...state,
            adsImportLoading: false,
            adsImportResult: action.result,
          }
        : state;
    case "close":
      return state.panel ? { ...state, panel: undefined } : state;
    case "reset":
      return EMPTY_BROWSE_SESSION;
  }
}

export function planBrowseOpen(
  protocol: string,
  target: Record<string, unknown>,
  label: string
): BrowseOpenPlan | undefined {
  const action = browseAction(protocol);
  if (!action) {
    return undefined;
  }
  const normalizedTarget = action.local
    ? { local: true }
    : protocol === "ads"
      ? withAdsTargetPort(target, adsTargetPort(target))
      : target;
  const panel: BrowsePanelState = {
    label,
    protocol,
    target: normalizedTarget,
    title: action.title,
    actionLabel: action.actionLabel,
    mode: action.mode,
  };
  const hasAdsDiscoveryEvidence =
    protocol === "ads" && adsPortBrowseEvidence(normalizedTarget).length > 0;
  return {
    panel,
    loading: protocol !== "ads" || !hasAdsDiscoveryEvidence,
    request:
      hasAdsDiscoveryEvidence
        ? undefined
        : browseRequestFor(panel, normalizedTarget),
  };
}

export function browseRequestFor(
  panel: BrowsePanelState,
  target: Record<string, unknown>
): BrowseSymbolsRequest | undefined {
  const action = browseAction(panel.protocol);
  return action
    ? { protocol: panel.protocol, target, kind: action.kind }
    : undefined;
}

export function normalizeEndpointBrowseTarget(
  protocol: string,
  params: Record<string, unknown>
): Record<string, unknown> {
  const connections = params.connections;
  if (
    (protocol === "opcua_client" || protocol === "ads") &&
    Array.isArray(connections) &&
    connections.length > 0
  ) {
    const first = connections[0];
    if (isRecord(first)) {
      if (protocol === "ads") {
        const matchingConnections = adsConnectionsForTarget(
          connections,
          first,
        );
        const imported = adsTagSelectionsFromConnections(connections, first);
        return {
          ...first,
          connections: connections.filter(isRecord),
          responding_ads_ports: adsDiscoveryPorts(
            matchingConnections.flatMap((connection) =>
              isRecord(connection) ? [connection.ams_port] : [],
            ),
          ),
          imported_ads_symbols: imported,
        };
      }
      return first;
    }
  }
  return params;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
