import {
  useCallback,
  useMemo,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";

import type { Node } from "@xyflow/react";
import type { CommSchemaResponse } from "../../communication/schemaForm";
import type { DiscoverCandidate } from "../offlineComm";
import {
  adsConnectionsForTarget,
  adsTagSelectionsFromConnections,
} from "../adsTagBatch";
import { browseAction } from "./browseActions";
import {
  buildDiscoverOrigins,
  discoverableProtocols,
  draftForDiscoveredCandidate,
  hostForDiscoveredRuntime,
  type DeviceDraft,
} from "./discoverPaneModel";
import { useDiscoverySession } from "./useDiscoverySession";

export function useDiscoverPaneLifecycle(
  post: (message: unknown) => void
) {
  const session = useDiscoverySession(post);
  const [open, setOpen] = useState(false);

  const close = useCallback(() => {
    session.close();
    setOpen(false);
  }, [session.close]);

  const show = useCallback(() => setOpen(true), []);
  const toggle = useCallback(() => {
    if (open) {
      close();
    } else {
      setOpen(true);
    }
  }, [close, open]);

  return {
    ...session,
    open,
    show,
    close,
    toggle,
  };
}

export function useDiscoverActions({
  nodes,
  schema,
  post,
  openBrowse,
  clearApplyResult,
  close,
  setSelectedId,
  setDraft,
  setEditMode,
}: {
  nodes: readonly Node[];
  schema?: CommSchemaResponse;
  post: (message: unknown) => void;
  openBrowse: (
    protocol: string,
    target: Record<string, unknown>,
    label: string
  ) => void;
  clearApplyResult: () => void;
  close: () => void;
  setSelectedId: Dispatch<SetStateAction<string | undefined>>;
  setDraft: Dispatch<SetStateAction<DeviceDraft | undefined>>;
  setEditMode: Dispatch<SetStateAction<boolean>>;
}) {
  const origins = useMemo(() => buildDiscoverOrigins(nodes), [nodes]);
  const protocols = useMemo(() => discoverableProtocols(schema), [schema]);
  const isOnCanvas = useCallback(
    (candidate: DiscoverCandidate) =>
      candidate.protocol === "ads" &&
      configuredAdsConnections(nodes, candidate.params).length > 0,
    [nodes]
  );

  const add = useCallback(
    (candidate: DiscoverCandidate) => {
      clearApplyResult();
      close();
      setSelectedId(undefined);
      if (browseAction(candidate.protocol)?.mode === "tags") {
        const target = candidate.protocol === "ads"
          ? mergeConfiguredAdsTags(candidate.params, nodes)
          : candidate.params;
        if (candidate.protocol === "ads") {
          post({
            type: "addAdsDevice",
            label: candidate.label,
            target: candidate.params,
          });
        }
        openBrowse(
          candidate.protocol,
          target,
          candidate.label || candidate.protocol
        );
        return;
      }
      setDraft(draftForDiscoveredCandidate(candidate, nodes));
    },
    [clearApplyResult, close, nodes, openBrowse, post, setDraft, setSelectedId]
  );

  const adopt = useCallback(
    (candidate: DiscoverCandidate) => {
      const host = hostForDiscoveredRuntime(candidate);
      if (host) {
        post({ type: "addHost", endpoint: host.endpoint, label: host.label });
      }
      close();
      setEditMode(false);
    },
    [close, post, setEditMode]
  );

  return { origins, protocols, add, isOnCanvas, adopt };
}

export function mergeConfiguredAdsTags(
  target: Record<string, unknown>,
  nodes: readonly Node[],
): Record<string, unknown> {
  const connections = nodes.flatMap((node) => {
    const params = isRecord(node.data.params) ? node.data.params : undefined;
    return params && Array.isArray(params.connections)
      ? params.connections
      : [];
  });
  const imported = adsTagSelectionsFromConnections(connections, target);
  return imported.length > 0
    ? { ...target, imported_ads_symbols: imported }
    : target;
}

function configuredAdsConnections(
  nodes: readonly Node[],
  target: Record<string, unknown>,
): Record<string, unknown>[] {
  const connections = nodes.flatMap((node) => {
    const params = isRecord(node.data.params) ? node.data.params : undefined;
    return params && Array.isArray(params.connections)
      ? params.connections
      : [];
  });
  return adsConnectionsForTarget(connections, target);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
