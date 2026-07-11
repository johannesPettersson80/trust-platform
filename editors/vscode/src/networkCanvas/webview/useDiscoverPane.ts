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

  const handoffToBrowse = useCallback(
    (candidate: DiscoverCandidate) => {
      const handedOff = session.handoffToBrowse(candidate);
      setOpen(false);
      return handedOff;
    },
    [session.handoffToBrowse]
  );

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
    handoffToBrowse,
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
  handoffToBrowse,
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
  handoffToBrowse: (candidate: DiscoverCandidate) => DiscoverCandidate;
  setSelectedId: Dispatch<SetStateAction<string | undefined>>;
  setDraft: Dispatch<SetStateAction<DeviceDraft | undefined>>;
  setEditMode: Dispatch<SetStateAction<boolean>>;
}) {
  const origins = useMemo(() => buildDiscoverOrigins(nodes), [nodes]);
  const protocols = useMemo(() => discoverableProtocols(schema), [schema]);

  const add = useCallback(
    (candidate: DiscoverCandidate) => {
      clearApplyResult();
      setSelectedId(undefined);
      if (browseAction(candidate.protocol)?.mode === "tags") {
        const browseCandidate =
          candidate.protocol === "ads"
            ? handoffToBrowse(candidate)
            : (close(), candidate);
        openBrowse(
          browseCandidate.protocol,
          browseCandidate.params,
          browseCandidate.label || browseCandidate.protocol
        );
        return;
      }
      close();
      setDraft(draftForDiscoveredCandidate(candidate, nodes));
    },
    [
      clearApplyResult,
      close,
      handoffToBrowse,
      nodes,
      openBrowse,
      setDraft,
      setSelectedId,
    ]
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

  return { origins, protocols, add, adopt };
}
