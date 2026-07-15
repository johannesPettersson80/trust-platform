import { useCallback, useReducer, useRef } from "react";

import type { DiscoverCandidate } from "../offlineComm";
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
  readonly error?: string;
}

type DiscoverySessionAction =
  | { readonly type: "reset" }
  | { readonly type: "scanStarted" }
  | { readonly type: "progress"; readonly row: DiscoverProgressRow }
  | {
      readonly type: "results";
      readonly candidates: readonly DiscoverCandidate[];
      readonly error?: string;
    };

export interface DiscoverySessionController extends DiscoverySessionState {
  readonly sessionId: string;
  prepareReady(): void;
  handleMessage(message: unknown): boolean;
  startScan(request: DiscoverRequest): void;
  close(): void;
}

const EMPTY_DISCOVERY_STATE: DiscoverySessionState = {
  scanning: false,
  progress: [],
  results: [],
  sessionCurrent: false,
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
      };
    case "progress":
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
        results: action.candidates,
        sessionCurrent: true,
        error: action.error,
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
        message.type !== "discoverResults"
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
            status: message.status === "done" ? "done" : "scanning",
            count:
              typeof message.count === "number" ? message.count : undefined,
          },
        });
      } else {
        dispatch({
          type: "results",
          candidates: Array.isArray(message.candidates)
            ? (message.candidates as DiscoverCandidate[])
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

function requestId(value: unknown): number | undefined {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? value
    : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
