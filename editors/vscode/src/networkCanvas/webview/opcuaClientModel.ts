// The OPC UA client browse pane's selection: leaves keyed by their raw node ID. The recovery a
// structured browse failure offers (`browse_error`) and the connection that the selected nodes are
// saved as (`opcua_add`) are the server's (`trust-lsp.devicesCanvasAct`; specification 25 §11.2).

import type { SymbolNode } from "../offlineComm";

// The structured browse/test failure the backend returns ({ code, message }), mapped to the one
// recovery action the UI should surface. "trust" is the explicit cert-trust path (B3 cert_untrusted).
export type OpcuaErrorAction = "trust" | "credentials" | "security" | "retry" | "none";

export interface OpcuaErrorView {
  code: string;
  action: OpcuaErrorAction;
  title: string;
  detail: string;
}

// A stable selection key for a browse leaf. Prefer the raw protocol id (OPC-UA node_id) so two
// leaves that share a display path are never conflated; fall back to the React id, then path. This is
// what B1 (the round-trippable node_id) is for — selection must not be keyed by the display path.
export function nodeKey(node: { node_id?: string; id?: string; path: string }): string {
  return node.node_id ?? node.id ?? node.path;
}

// Flatten a browse tree to the leaves whose stable key is selected, in tree order. Used by every
// browse flow (the App then extracts the per-protocol payload), so selection is node-identity based.
export function selectedLeaves(
  tree: SymbolNode[] | undefined,
  selectedKeys: ReadonlySet<string>
): SymbolNode[] {
  const out: SymbolNode[] = [];
  const walk = (nodes: SymbolNode[] | undefined): void => {
    for (const n of nodes ?? []) {
      if (n.children?.length) {
        walk(n.children);
      } else if (selectedKeys.has(nodeKey(n))) {
        out.push(n);
      }
    }
  };
  walk(tree);
  return out;
}
