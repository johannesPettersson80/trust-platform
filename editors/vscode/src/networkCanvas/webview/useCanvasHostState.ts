import { useEffect, useState } from "react";

import type {
  CommApplyResponse,
  CommSchemaResponse,
} from "../../communication/schemaForm";
import type { NCGraph } from "./types";

const EMPTY_GRAPH: NCGraph = {
  kind: "graph",
  title: "Devices & Connections",
  summary: "",
  hosts: [],
  links: [],
  external: [],
  faults: [],
};

export function useCanvasHostState({
  handleDiscoveryMessage,
  handleBrowseMessage,
  prepareDiscoveryReady,
  onFocusNode,
  onApplyResult,
}: {
  handleDiscoveryMessage: (message: unknown) => boolean;
  handleBrowseMessage: (message: unknown) => boolean;
  prepareDiscoveryReady: () => void;
  onFocusNode: (nodeId: string) => void;
  onApplyResult: (result: CommApplyResponse | undefined) => void;
}) {
  const [graph, setGraph] = useState<NCGraph>(
    () =>
      (typeof window !== "undefined" &&
        (window as { __NC__?: NCGraph }).__NC__) ||
      EMPTY_GRAPH
  );
  const [schema, setSchema] = useState<CommSchemaResponse | undefined>();
  const [reachable, setReachable] = useState(false);
  const [setupMessage, setSetupMessage] = useState<string | undefined>();

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const message = event.data;
      if (
        handleDiscoveryMessage(message) ||
        handleBrowseMessage(message)
      ) {
        return;
      }
      if (message && message.type === "graph" && message.graph) {
        setGraph(message.graph as NCGraph);
      }
      if (message && message.type === "meta") {
        setSchema(message.schema as CommSchemaResponse | undefined);
        onApplyResult(message.applyResult as CommApplyResponse | undefined);
        setReachable(Boolean(message.reachable));
        setSetupMessage(
          typeof message.setupMessage === "string"
            ? message.setupMessage
            : undefined
        );
      }
      if (
        message &&
        message.type === "focusNode" &&
        typeof message.nodeId === "string"
      ) {
        onFocusNode(message.nodeId);
      }
    };
    window.addEventListener("message", onMessage);
    prepareDiscoveryReady();
    return () => window.removeEventListener("message", onMessage);
  }, [
    handleBrowseMessage,
    handleDiscoveryMessage,
    onApplyResult,
    onFocusNode,
    prepareDiscoveryReady,
  ]);

  return {
    graph,
    schema,
    reachable,
    setupMessage,
  };
}
