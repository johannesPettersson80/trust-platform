import { useCallback, useReducer, useRef } from "react";

import type { DiscoverCandidate } from "../offlineComm";
import {
  isDiscoveryErrorCode,
  type DiscoveryErrorCode,
} from "../discoveryErrors";
import type {
  AdsServiceProbeResult,
  AdsServiceProbeViewState,
} from "../adsServiceProbeModel";
import type {
  DiscoverProgressRow,
  DiscoverRequest,
} from "./discoverPaneModel";

interface DiscoveryMessageEnvelope {
  readonly sessionId: string;
  readonly requestId: number;
}

export interface DiscoverySessionState {
  readonly scanning: boolean;
  readonly progress: readonly DiscoverProgressRow[];
  readonly results: readonly DiscoverCandidate[];
  readonly sessionCurrent: boolean;
  readonly terminal: boolean;
  readonly adsServiceProbes: Readonly<Record<string, AdsServiceProbeViewState>>;
  readonly warning?: string;
  readonly warningDetails?: readonly string[];
  readonly error?: string;
  readonly errorDetails?: readonly string[];
  readonly errorCode?: DiscoveryErrorCode;
}

export type DiscoverySessionAction =
  | { readonly type: "reset" }
  | { readonly type: "scanStarted" }
  | { readonly type: "progress"; readonly row: DiscoverProgressRow }
  | {
      readonly type: "results";
      readonly candidates: readonly DiscoverCandidate[];
      readonly warning?: string;
      readonly warningDetails?: readonly string[];
      readonly error?: string;
      readonly errorDetails?: readonly string[];
      readonly errorCode?: DiscoveryErrorCode;
    }
  | { readonly type: "adsProbeStarted"; readonly candidateId: string }
  | {
      readonly type: "adsProbeProgress";
      readonly candidateId: string;
      readonly port: number;
    }
  | {
      readonly type: "adsProbeResults";
      readonly candidateId: string;
      readonly results: readonly AdsServiceProbeResult[];
      readonly error?: string;
    };

export interface DiscoverySessionController extends DiscoverySessionState {
  readonly sessionId: string;
  prepareReady(): void;
  handleMessage(message: unknown): boolean;
  startScan(request: DiscoverRequest): void;
  probeAdsServices(
    candidate: DiscoverCandidate,
    ports: readonly number[],
    origin: string
  ): void;
  handoffToBrowse(candidate: DiscoverCandidate): DiscoverCandidate;
  reset(): void;
  close(): void;
}

export function discoveryProgressStatus(
  status: unknown
): DiscoverProgressRow["status"] {
  return status === "done" || status === "failed" ? status : "scanning";
}

const EMPTY_DISCOVERY_STATE: DiscoverySessionState = {
  scanning: false,
  progress: [],
  results: [],
  sessionCurrent: false,
  terminal: false,
  adsServiceProbes: {},
};

export function reduceDiscoverySessionState(
  state: DiscoverySessionState,
  action: DiscoverySessionAction
): DiscoverySessionState {
  switch (action.type) {
    case "reset":
      return EMPTY_DISCOVERY_STATE;
    case "scanStarted":
      return {
        scanning: true,
        progress: [],
        results: [],
        sessionCurrent: true,
        terminal: false,
        adsServiceProbes: {},
      };
    case "progress":
      if (state.terminal) {
        return state;
      }
      return {
        ...state,
        scanning: true,
        sessionCurrent: true,
        progress: [
          ...state.progress.filter((row) => row.label !== action.row.label),
          action.row,
        ],
      };
    case "results":
      return {
        ...state,
        scanning: false,
        progress: action.error
          ? state.progress.map((row) =>
              row.status === "scanning" ? { ...row, status: "failed" } : row
            )
          : state.progress,
        results: action.candidates,
        sessionCurrent: true,
        terminal: true,
        warning: action.warning,
        warningDetails: action.warningDetails,
        error: action.error,
        errorDetails: action.errorDetails,
        errorCode: action.errorCode,
      };
    case "adsProbeStarted":
      return {
        ...state,
        adsServiceProbes: {
          ...Object.fromEntries(
            Object.entries(state.adsServiceProbes).map(([candidateId, probe]) => [
              candidateId,
              candidateId !== action.candidateId && probe.probing
                ? {
                    ...probe,
                    probing: false,
                    currentPort: undefined,
                    completed: true,
                    error: "Check canceled because another ADS device check started.",
                  }
                : probe,
            ])
          ),
          [action.candidateId]: {
            probing: true,
            results: [],
            completed: false,
          },
        },
      };
    case "adsProbeProgress":
      if (state.adsServiceProbes[action.candidateId]?.completed) {
        return state;
      }
      return {
        ...state,
        adsServiceProbes: {
          ...state.adsServiceProbes,
          [action.candidateId]: {
            probing: true,
            results: state.adsServiceProbes[action.candidateId]?.results ?? [],
            currentPort: action.port,
            completed: false,
          },
        },
      };
    case "adsProbeResults":
      return {
        ...state,
        adsServiceProbes: {
          ...state.adsServiceProbes,
          [action.candidateId]: {
            probing: false,
            results: action.results,
            currentPort: undefined,
            completed: true,
            error: action.error,
          },
        },
      };
  }
}

export function isCurrentDiscoveryEnvelope(
  message: unknown,
  sessionId: string,
  requestId: number
): message is Record<string, unknown> & DiscoveryMessageEnvelope {
  return (
    isRecord(message) &&
    message.sessionId === sessionId &&
    message.requestId === requestId
  );
}

/**
 * Owns the ephemeral Discover scan identity and UI state.
 *
 * The webview session id distinguishes a freshly mounted iframe from work that
 * was still running for its predecessor. The monotonically increasing request
 * id distinguishes scans, closes, and resets inside one mounted webview.
 */
export function useDiscoverySession(
  post: (message: unknown) => void
): DiscoverySessionController {
  const sessionIdRef = useRef<string | undefined>(undefined);
  if (!sessionIdRef.current) {
    sessionIdRef.current = createDiscoverySessionId();
  }
  const sessionId = sessionIdRef.current;
  const requestIdRef = useRef(0);
  const sessionCurrentRef = useRef(false);
  const [state, dispatch] = useReducer(
    reduceDiscoverySessionState,
    EMPTY_DISCOVERY_STATE
  );

  const resetLocal = useCallback(() => {
    sessionCurrentRef.current = false;
    dispatch({ type: "reset" });
  }, []);

  const prepareReady = useCallback(() => {
    const requestId = ++requestIdRef.current;
    resetLocal();
    post({ type: "ready", sessionId, requestId });
  }, [post, resetLocal, sessionId]);

  const handleMessage = useCallback(
    (message: unknown): boolean => {
      if (!isRecord(message) || typeof message.type !== "string") {
        return false;
      }
      if (message.type === "discoverReset") {
        if (message.sessionId !== sessionId) {
          return true;
        }
        const resetRequestId = requestId(message.requestId);
        if (
          resetRequestId !== undefined &&
          resetRequestId < requestIdRef.current
        ) {
          return true;
        }
        requestIdRef.current =
          (resetRequestId ?? requestIdRef.current) + 1;
        resetLocal();
        return true;
      }
      if (
        message.type !== "discoverProgress" &&
        message.type !== "discoverResults" &&
        message.type !== "adsServiceProbeProgress" &&
        message.type !== "adsServiceProbeResults"
      ) {
        return false;
      }
      if (
        !isCurrentDiscoveryEnvelope(
          message,
          sessionId,
          requestIdRef.current
        )
      ) {
        return true;
      }
      sessionCurrentRef.current = true;
      if (message.type === "discoverProgress") {
        dispatch({
          type: "progress",
          row: {
            protocol: String(message.protocol ?? ""),
            label: String(message.label ?? ""),
            status: discoveryProgressStatus(message.status),
            count:
              typeof message.count === "number" ? message.count : undefined,
          },
        });
      } else if (message.type === "discoverResults") {
        dispatch({
          type: "results",
          candidates: Array.isArray(message.candidates)
            ? (message.candidates as DiscoverCandidate[])
            : [],
          warning:
            typeof message.warning === "string" ? message.warning : undefined,
          warningDetails: Array.isArray(message.warningDetails)
            ? message.warningDetails.filter(
                (detail): detail is string =>
                  typeof detail === "string" && detail.trim().length > 0
              )
            : undefined,
          error: typeof message.error === "string" ? message.error : undefined,
          errorDetails: Array.isArray(message.errorDetails)
            ? message.errorDetails.filter(
                (detail): detail is string =>
                  typeof detail === "string" && detail.trim().length > 0
              )
            : undefined,
          errorCode: isDiscoveryErrorCode(message.errorCode)
            ? message.errorCode
            : undefined,
        });
      } else if (
        message.type === "adsServiceProbeProgress" &&
        typeof message.candidateId === "string" &&
        typeof message.port === "number" &&
        Number.isSafeInteger(message.port)
      ) {
        dispatch({
          type: "adsProbeProgress",
          candidateId: message.candidateId,
          port: message.port,
        });
      } else if (typeof message.candidateId === "string") {
        dispatch({
          type: "adsProbeResults",
          candidateId: message.candidateId,
          results: Array.isArray(message.results)
            ? (message.results as AdsServiceProbeResult[])
            : [],
          error: typeof message.error === "string" ? message.error : undefined,
        });
      }
      return true;
    },
    [resetLocal, sessionId]
  );

  const startScan = useCallback(
    (request: DiscoverRequest) => {
      const requestId = ++requestIdRef.current;
      sessionCurrentRef.current = true;
      dispatch({ type: "scanStarted" });
      post({ type: "discover", sessionId, requestId, request });
    },
    [post, sessionId]
  );

  const probeAdsServices = useCallback(
    (
      candidate: DiscoverCandidate,
      ports: readonly number[],
      origin: string
    ) => {
      dispatch({ type: "adsProbeStarted", candidateId: candidate.id });
      post({
        type: "probeAdsServices",
        sessionId,
        requestId: requestIdRef.current,
        origin,
        candidate,
        ports,
      });
    },
    [post, sessionId]
  );

  const handoffToBrowse = useCallback(
    (candidate: DiscoverCandidate) => {
      const requestId = requestIdRef.current;
      const leaseId = candidate.originRuntimeId
        ? createDiscoveryBrowseLeaseId()
        : undefined;
      post({
        type: "handoffDiscoveryToBrowse",
        sessionId,
        requestId,
        protocol: candidate.protocol,
        originRuntimeId: candidate.originRuntimeId,
        leaseId,
      });
      requestIdRef.current += 1;
      resetLocal();
      return leaseId
        ? {
            ...candidate,
            params: {
              ...candidate.params,
              discovery_origin_lease_id: leaseId,
            },
          }
        : candidate;
    },
    [post, resetLocal, sessionId]
  );

  const close = useCallback(() => {
    const canceledRequestId = requestIdRef.current;
    const shouldNotifyHost = sessionCurrentRef.current;
    requestIdRef.current += 1;
    resetLocal();
    if (shouldNotifyHost) {
      post({
        type: "cancelDiscover",
        sessionId,
        requestId: canceledRequestId,
      });
    }
  }, [post, resetLocal, sessionId]);

  return {
    ...state,
    sessionId,
    prepareReady,
    handleMessage,
    startScan,
    probeAdsServices,
    handoffToBrowse,
    reset: close,
    close,
  };
}

function createDiscoverySessionId(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  return `network-canvas-${Date.now().toString(36)}-${Math.random()
    .toString(36)
    .slice(2)}`;
}

function createDiscoveryBrowseLeaseId(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  return `discovery-browse-${Date.now().toString(36)}-${Math.random()
    .toString(36)
    .slice(2)}`;
}

function requestId(value: unknown): number | undefined {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? value
    : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
