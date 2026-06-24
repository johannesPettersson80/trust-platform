// Pure model for the OPC-UA CLIENT browse→select→save flow (no React/vscode imports, unit-tested).
// truST reads selected nodes from an EXTERNAL OPC-UA server (peer_link); this is the opposite of the
// OPC-UA *server* expose flow. Backend contract (commit ca5ba2d20): browse leaves carry a raw `node_id`
// (round-trips into apply), a friendly `data_type`, and browse/test failures carry a structured
// `{ code, message }` error. See docs/internal/design/opcua-client-network-canvas-plan.md.

import type { SymbolNode } from "../offlineComm";

// One OPC-UA point persisted under a connection (matches comm.apply connections[].points[]).
export interface OpcuaPoint {
  var: string; // truST global the node maps to, e.g. "global.Temperature"
  node_id: string; // raw OPC-UA NodeId, e.g. "ns=2;i=1"
  type: string; // IEC data type, e.g. "double"
  access: "read" | "read_write";
}

// A saved opcua_client connection (matches comm.apply connections[]).
export interface OpcuaConnection {
  name: string;
  endpoint_url: string;
  security_policy: string;
  security_mode: string;
  auth: string;
  username?: string;
  password?: string;
  trust_server_certificate: boolean;
  points: OpcuaPoint[];
}

// The structured browse/test failure the backend returns ({ code, message }), mapped to the one
// recovery action the UI should surface. "trust" is the explicit cert-trust path (B3 cert_untrusted).
export type OpcuaErrorAction = "trust" | "credentials" | "security" | "retry" | "none";

export interface OpcuaErrorView {
  code: string;
  action: OpcuaErrorAction;
  title: string;
  detail: string;
}

// ST variable name for a browsed node: "global." + the path (or name) with non-identifier chars
// folded to "_". Deterministic so re-browsing the same node yields the same var.
export function deriveOpcuaVarName(node: Pick<SymbolNode, "name" | "path">): string {
  const core = (node.path || node.name || "node")
    .replace(/[^A-Za-z0-9_]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return `global.${core || "node"}`;
}

// Friendly type for display + apply: prefer the backend's resolved data_type (B2), else the raw
// OPC-UA type NodeId (e.g. "i=11"), else empty.
export function opcuaDisplayType(node: Pick<SymbolNode, "type" | "data_type">): string {
  return node.data_type ?? node.type ?? "";
}

// A node can only be enabled for writes when the server reports it writable AND the user opted in.
export function opcuaAccess(
  writable: boolean | undefined,
  allowWrites: boolean
): "read" | "read_write" {
  return writable === true && allowWrites ? "read_write" : "read";
}

// Build a persisted point from a browsed leaf. Returns undefined when the leaf lacks a raw node_id
// (cannot be saved honestly — guards against the pre-B1 sanitized-only id).
export function opcuaPointFromNode(
  node: SymbolNode,
  allowWrites: boolean
): OpcuaPoint | undefined {
  if (!node.node_id) {
    return undefined;
  }
  return {
    var: deriveOpcuaVarName(node),
    node_id: node.node_id,
    type: opcuaDisplayType(node) || "string",
    access: opcuaAccess(node.writable, allowWrites),
  };
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

// Connection name derived from a server label/endpoint, slug-safe and stable.
export function deriveOpcuaConnectionName(label: string, endpointUrl: string): string {
  const base = (label && label !== "opcua_client" ? label : endpointUrl) || "opcua";
  return (
    base
      .replace(/^opc\.tcp:\/\//, "")
      .replace(/[^A-Za-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .toLowerCase()
      .slice(0, 48) || "opcua"
  );
}

// Build the full connection payload for comm.apply from a browse target + selected nodes.
// `target` is the discovered/configured connection params carried through browse.
export function buildOpcuaConnection(
  target: Record<string, unknown>,
  label: string,
  selected: SymbolNode[],
  allowWrites: boolean
): OpcuaConnection | undefined {
  const endpoint_url = str(target.endpoint_url);
  if (!endpoint_url) {
    return undefined;
  }
  const points = selected
    .map((n) => opcuaPointFromNode(n, allowWrites))
    .filter((p): p is OpcuaPoint => p !== undefined);
  if (points.length === 0) {
    return undefined;
  }
  const conn: OpcuaConnection = {
    name: deriveOpcuaConnectionName(label, endpoint_url),
    endpoint_url,
    security_policy: str(target.security_policy) || "none",
    security_mode: str(target.security_mode) || "none",
    auth: str(target.auth) || "anonymous",
    trust_server_certificate: target.trust_server_certificate === true,
    points,
  };
  const username = str(target.username);
  const password = str(target.password);
  if (conn.auth === "username") {
    if (username) conn.username = username;
    if (password) conn.password = password;
  }
  return conn;
}

// Map a structured backend error to the single recovery action the UI should offer.
export function classifyOpcuaBrowseError(error: {
  code?: string;
  message?: string;
}): OpcuaErrorView {
  const code = error.code ?? "";
  const message = error.message ?? "OPC UA browse failed.";
  switch (code) {
    case "cert_untrusted":
      return {
        code,
        action: "trust",
        title: "Server certificate not trusted",
        detail: message,
      };
    case "auth_required":
      return {
        code,
        action: "credentials",
        title: "Authentication required",
        detail: message,
      };
    case "unsupported_security_profile":
      return {
        code,
        action: "security",
        title: "Unsupported security profile",
        detail: message,
      };
    case "endpoint_unreachable":
      return { code, action: "retry", title: "Server unreachable", detail: message };
    case "browse_denied":
      return { code, action: "none", title: "Browse denied", detail: message };
    default:
      return { code: code || "unknown", action: "none", title: "OPC UA browse failed", detail: message };
  }
}

function str(value: unknown): string {
  return typeof value === "string" ? value : "";
}
