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
import type { DiscoveredAdsFacts } from "../canvasCommands";
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
  discoveredAds,
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
  /** What the server says about each ADS device of the last search (`trust-lsp.devicesTopology`). */
  discoveredAds: readonly DiscoveredAdsFacts[];
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
  const factsOf = useCallback(
    (candidate: DiscoverCandidate) =>
      discoveredAds.find((facts) => facts.id === candidate.id),
    [discoveredAds]
  );
  const isOnCanvas = useCallback(
    (candidate: DiscoverCandidate) =>
      candidate.protocol === "ads" && factsOf(candidate)?.on_canvas === true,
    [factsOf]
  );
  // Why a result cannot be added now is the server's (`trust-lsp.devicesTopology`).
  const disabledReason = useCallback(
    (candidate: DiscoverCandidate) => factsOf(candidate)?.disabled_reason,
    [factsOf]
  );

  const add = useCallback(
    (candidate: DiscoverCandidate) => {
      clearApplyResult();
      close();
      setSelectedId(undefined);
      if (browseAction(candidate.protocol)?.mode === "tags") {
        const imported = factsOf(candidate)?.imported_ads_symbols ?? [];
        const target =
          candidate.protocol === "ads" && imported.length > 0
            ? { ...candidate.params, imported_ads_symbols: imported }
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
      setDraft(draftForDiscoveredCandidate(candidate, factsOf(candidate), nodes));
    },
    [clearApplyResult, close, factsOf, nodes, openBrowse, post, setDraft, setSelectedId]
  );

  const adopt = useCallback(
    (candidate: DiscoverCandidate) => {
      const host = hostForDiscoveredRuntime(factsOf(candidate));
      if (host) {
        post({ type: "addHost", endpoint: host.endpoint, label: host.label });
      }
      close();
      setEditMode(false);
    },
    [close, factsOf, post, setEditMode]
  );

  return { origins, protocols, add, isOnCanvas, disabledReason, adopt };
}
