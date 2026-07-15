import { useCallback, useReducer, useRef } from "react";

import type { RoutePlan, SymbolNode } from "../offlineComm";
import {
  normalizeAdsTagSelections,
  type AdsTagPortSelection,
  type AdsTagPortPath,
  type AdsTagBatchImportResult,
} from "../adsTagBatch";
import type { InspectorNode } from "./NodeInspector";
import {
  EMPTY_BROWSE_SESSION,
  browseRequestFor,
  isCurrentBrowseMessage,
  normalizeEndpointBrowseTarget,
  planBrowseOpen,
  reduceBrowseSessionState,
  type BrowsePanelState,
  type BrowseSessionState,
  type BrowseSymbolsRequest,
} from "./browseSessionModel";
import { classifyBrowseError } from "./browseErrorModel";
import { buildOpcuaConnection, selectedLeaves } from "./opcuaClientModel";
import { withAdsTargetPort } from "./adsTargetPort";

export interface BrowseSessionController extends BrowseSessionState {
  open(protocol: string, target: Record<string, unknown>, label: string): void;
  openNode(node: InspectorNode): void;
  handleMessage(message: unknown): boolean;
  browseTarget(target: Record<string, unknown>): void;
  trustCertificate(): void;
  createRoute(port?: number): void;
  copy(text: string): void;
  addTags(keys: string[], writable: boolean): void;
  addAdsTags(
    selections: readonly AdsTagPortSelection[],
    writable: boolean,
    changedPath: string,
  ): void;
  removeAdsTag(selection: AdsTagPortPath): void;
  close(): void;
}

/** Owns the browse drawer's target, request, result, and save lifecycle. */
export function useBrowseSession(
  post: (message: unknown) => void,
  onBeforeOpen: () => void
): BrowseSessionController {
  const [state, dispatch] = useReducer(
    reduceBrowseSessionState,
    EMPTY_BROWSE_SESSION
  );
  const panelRef = useRef<BrowsePanelState | undefined>(undefined);
  const treeRef = useRef<SymbolNode[] | undefined>(undefined);
  const protocolRef = useRef("");
  const sessionIdRef = useRef<string | undefined>(undefined);
  if (!sessionIdRef.current) {
    sessionIdRef.current = createBrowseSessionId();
  }
  const sessionId = sessionIdRef.current;
  const requestIdRef = useRef(0);
  const adsImportRequestIdRef = useRef(0);

  const postBrowseRequest = useCallback(
    (request: BrowseSymbolsRequest) => {
      const browseRequestId = ++requestIdRef.current;
      post({
        type: "browseSymbols",
        browseSessionId: sessionId,
        browseRequestId,
        ...request,
      });
    },
    [post, sessionId]
  );

  const open = useCallback(
    (protocol: string, target: Record<string, unknown>, label: string) => {
      const plan = planBrowseOpen(protocol, target, label);
      if (!plan) {
        return;
      }
      onBeforeOpen();
      requestIdRef.current += 1;
      adsImportRequestIdRef.current += 1;
      panelRef.current = plan.panel;
      treeRef.current = undefined;
      protocolRef.current = protocol;
      dispatch({
        type: "open",
        panel: plan.panel,
        loading: plan.loading,
      });
      if (plan.request) {
        postBrowseRequest(plan.request);
      }
    },
    [onBeforeOpen, postBrowseRequest]
  );

  const openNode = useCallback(
    (node: InspectorNode) => {
      const protocol = String(node.data.protocol ?? "");
      const params =
        (node.data.params as Record<string, unknown> | undefined) ?? {};
      open(
        protocol,
        normalizeEndpointBrowseTarget(protocol, params),
        String(node.data.name ?? node.data.label ?? protocol)
      );
    },
    [open]
  );

  const handleMessage = useCallback((message: unknown): boolean => {
    if (!isRecord(message)) {
      return false;
    }
    if (message.type === "browseReset") {
      requestIdRef.current += 1;
      adsImportRequestIdRef.current += 1;
      panelRef.current = undefined;
      treeRef.current = undefined;
      protocolRef.current = "";
      dispatch({ type: "reset" });
      return true;
    }
    if (message.type === "adsTagsResult") {
      if (
        message.browseSessionId !== sessionId ||
        message.adsImportRequestId !== adsImportRequestIdRef.current ||
        !isRecord(message.result)
      ) {
        return true;
      }
      dispatch({
        type: "adsImportResult",
        result: message.result as unknown as AdsTagBatchImportResult,
      });
      return true;
    }
    if (message.type !== "symbolTree") {
      return false;
    }
    if (!isCurrentBrowseMessage(message, sessionId, requestIdRef.current)) {
      return true;
    }
    const tree = Array.isArray(message.tree)
      ? (message.tree as SymbolNode[])
      : [];
    treeRef.current = tree;
    dispatch({
      type: "result",
      tree,
      routeMissing: Boolean(message.routeMissing),
      routePlan: message.routePlan as RoutePlan | undefined,
      error: message.error
        ? classifyBrowseError(
            protocolRef.current,
            message.error as { code?: string; message?: string }
          )
        : undefined,
    });
    return true;
  }, [sessionId]);

  const browseTarget = useCallback(
    (target: Record<string, unknown>) => {
      const panel = panelRef.current;
      if (!panel) {
        return;
      }
      const request = browseRequestFor(panel, target);
      if (!request) {
        return;
      }
      panelRef.current = { ...panel, target };
      treeRef.current = undefined;
      protocolRef.current = panel.protocol;
      dispatch({ type: "request", target });
      postBrowseRequest(request);
    },
    [postBrowseRequest]
  );

  const trustCertificate = useCallback(() => {
    const panel = panelRef.current;
    if (!panel) {
      return;
    }
    const target = { ...panel.target, trust_server_certificate: true };
    panelRef.current = { ...panel, target };
    treeRef.current = undefined;
    protocolRef.current = panel.protocol;
    dispatch({ type: "request", target });
    postBrowseRequest({
      protocol: panel.protocol,
      target,
      kind: "nodes",
    });
  }, [postBrowseRequest]);

  const createRoute = useCallback((port?: number) => {
    const panel = panelRef.current;
    if (panel) {
      post({
        type: "createRoute",
        protocol: panel.protocol,
        target:
          panel.protocol === "ads" && port
            ? withAdsTargetPort(panel.target, port)
            : panel.target,
      });
    }
  }, [post]);

  const copy = useCallback(
    (text: string) => post({ type: "copyText", text }),
    [post]
  );

  const close = useCallback(() => {
    requestIdRef.current += 1;
    adsImportRequestIdRef.current += 1;
    panelRef.current = undefined;
    dispatch({ type: "close" });
  }, []);

  const addAdsTags = useCallback(
    (
      selections: readonly AdsTagPortSelection[],
      writable: boolean,
      changedPath: string,
    ) => {
      const panel = panelRef.current;
      const normalized = normalizeAdsTagSelections(selections);
      if (!panel || panel.protocol !== "ads" || normalized.length === 0) {
        return;
      }
      const adsImportRequestId = ++adsImportRequestIdRef.current;
      dispatch({ type: "adsImportStarted" });
      post({
        type: "addAdsTagsBatch",
        browseSessionId: sessionId,
        adsImportRequestId,
        target: panel.target,
        selections: normalized,
        writable,
        changedPath,
      });
    },
    [post, sessionId]
  );

  const removeAdsTag = useCallback(
    (selection: AdsTagPortPath) => {
      const panel = panelRef.current;
      if (!panel || panel.protocol !== "ads") {
        return;
      }
      const adsImportRequestId = ++adsImportRequestIdRef.current;
      dispatch({ type: "adsImportStarted" });
      post({
        type: "removeAdsTag",
        browseSessionId: sessionId,
        adsImportRequestId,
        target: panel.target,
        selection,
      });
    },
    [post, sessionId]
  );

  const addTags = useCallback(
    (keys: string[], writable: boolean) => {
      const panel = panelRef.current;
      if (panel && keys.length > 0) {
        const nodes = selectedLeaves(treeRef.current, new Set(keys));
        if (panel.protocol === "opcua_client") {
          const connection = buildOpcuaConnection(
            panel.target,
            panel.label,
            nodes,
            writable
          );
          if (connection) {
            post({ type: "addOpcuaConnection", connection });
          }
        } else if (panel.protocol === "ethercat") {
          post({
            type: "addEthercatChannels",
            target: panel.target,
            paths: nodes.map((node) => node.path),
          });
        } else {
          post({
            type: panel.mode === "expose" ? "addExpose" : "addTags",
            protocol: panel.protocol,
            target: panel.target,
            paths: nodes.map((node) => node.path),
            writable,
          });
        }
      }
      close();
    },
    [close, post]
  );

  return {
    ...state,
    open,
    openNode,
    handleMessage,
    browseTarget,
    trustCertificate,
    createRoute,
    copy,
    addTags,
    addAdsTags,
    removeAdsTag,
    close,
  };
}

function createBrowseSessionId(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  return `network-canvas-browse-${Date.now().toString(36)}-${Math.random()
    .toString(36)
    .slice(2)}`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
