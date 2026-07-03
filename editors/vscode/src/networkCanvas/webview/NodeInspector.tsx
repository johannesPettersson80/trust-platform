import React, { useEffect, useMemo, useState } from "react";
import { healthColor, roleWord } from "./nodes";
import { protocolColor, protocolName } from "./protocolMeta";
import { t, tint } from "./theme";
import { buildParams, Field, valuesFor } from "./SchemaFields";
import { browseAction } from "./browseActions";
import {
  runtimeNodeControlLayout,
  runtimeNodeControls,
  type RuntimeNodeControl,
} from "./runtimeNodeControls";
import { LOCAL_RUNTIME_NODE_ID } from "./types";
import type {
  CommApplyResponse,
  CommFieldSchema,
  CommProtocolSchema,
  CommSchemaResponse,
} from "../../communication/schemaForm";

// §4 D + settings UX (2026-06-17/18): single-click a node → ONE side panel. It opens on a
// read-only SUMMARY (what it is + its current settings); an Edit button switches to the
// schema-driven form. Editing is file-based, so it works whether the device is stopped,
// offline, or running (Save writes config; "Apply live" pushes to a running runtime only when
// one is online). Non-configurable nodes (host/container/external) are summary-only.

export interface InspectorNode {
  id: string;
  type?: string;
  data: Record<string, unknown>;
}

interface Props {
  node: InspectorNode;
  schema?: CommSchemaResponse;
  params?: Record<string, unknown>; // the endpoint's current config values
  reachable: boolean; // a runtime is online → "Apply live" is possible
  applyResult?: CommApplyResponse;
  onClose: () => void;
  onFocus: (nodeId: string) => void;
  onBrowse?: (node: InspectorNode) => void; // §0.5.2 browse what the endpoint exposes (tags/channels/globals)
  post: (message: unknown) => void;
}

function str(value: unknown): string {
  return value === undefined || value === null ? "" : String(value);
}

function healthLabel(health: string): string {
  switch (health) {
    case "connected":
      return "Connected";
    case "stopped":
      return "Stopped";
    case "configured_policy":
      return "Configured";
    case "disabled":
      return "Disabled";
    case "not_configured":
      return "Not configured";
    case "runtime_unreachable":
      return "Runtime unreachable";
    case "auth_failed":
      return "Authentication failed";
    case "degraded":
      return "Degraded";
    case "error":
      return "Error";
    case "pending":
      return "Pending";
    case "simulate":
      return "Simulator";
    case "unknown":
      return "Unknown";
    default:
      return health
        ? health.replace(/_/g, " ").replace(/\b\w/g, (char) => char.toUpperCase())
        : "Unknown";
  }
}

function stateSummary(health: string, detail: string): string {
  const label = healthLabel(health);
  const cleanDetail = normalizedConfiguredDetail(detail);
  if (!cleanDetail) {
    return label;
  }
  const normalizedDetail = cleanDetail.toLowerCase();
  if (
    normalizedDetail === label.toLowerCase() ||
    normalizedDetail === health.trim().toLowerCase()
  ) {
    return label;
  }
  return `${label} · ${cleanDetail}`;
}

function normalizedConfiguredDetail(detail: string): string {
  return detail
    .trim()
    .replace(/^Configured in [^;]+;\s*/i, "")
    .replace(/^Loaded from project files;\s*/i, "")
    .trim();
}

function runtimeModeLabel(mode: string): string {
  switch (mode.trim().toLowerCase()) {
    case "simulate":
    case "simulator":
      return "Simulator";
    case "managed":
      return "Managed";
    case "remote":
      return "Remote";
    case "attached":
      return "Attached";
    case "local":
      return "Local";
    case "":
    case "stopped":
    case "running":
    case "connected":
    case "online":
    case "pending":
    case "degraded":
    case "error":
    case "auth_failed":
    case "runtime_unreachable":
    case "unknown":
      return "";
    default:
      return mode.replace(/_/g, " ").replace(/\b\w/g, (char) => char.toUpperCase());
  }
}

function testParamsFor(protocol: string, params?: Record<string, unknown>): Record<string, unknown> {
  if (!params) {
    return {};
  }
  const connections = params.connections;
  if (protocol === "opcua_client" && Array.isArray(connections) && connections.length > 0) {
    const first = connections[0];
    if (first && typeof first === "object" && !Array.isArray(first)) {
      return first as Record<string, unknown>;
    }
  }
  return params;
}

function isRecordValue(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isRuntimeAuthTokenFailure(node: InspectorNode): boolean {
  if (node.type !== "runtime") {
    return false;
  }
  const endpoint = str(node.data.controlEndpoint);
  if (!endpoint || node.id === LOCAL_RUNTIME_NODE_ID || node.data.managed === true) {
    return false;
  }
  const health = str(node.data.health).toLowerCase();
  const detail = str(node.data.detail).toLowerCase();
  return (
    health === "auth_failed" ||
    (detail.includes("authentication failed") && detail.includes("auth token")) ||
    detail.includes("no auth token provided") ||
    detail.includes("auth token rejected")
  );
}

function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

function connectionSummary(value: unknown): string {
  let connections: unknown[] = [];
  if (Array.isArray(value)) {
    connections = value;
  } else if (typeof value === "string" && value.trim().startsWith("[")) {
    try {
      const parsed = JSON.parse(value) as unknown;
      connections = Array.isArray(parsed) ? parsed : [];
    } catch {
      connections = [];
    }
  }
  if (connections.length === 0) {
    return "";
  }
  return connections
    .map((connection, index) => {
      if (!isRecordValue(connection)) {
        return `connection ${index + 1}`;
      }
      const name = str(connection.name) || `connection ${index + 1}`;
      const endpoint =
        str(connection.endpoint_url) ||
        str(connection.host) ||
        str(connection.address) ||
        str(connection.broker);
      const points = Array.isArray(connection.points) ? connection.points : [];
      const mappings = points
        .filter(isRecordValue)
        .map((point) => pointMapping(point as Record<string, unknown>))
        .filter(Boolean);
      const head = [name, endpoint].filter(Boolean).join(" · ");
      return mappings.length === 0 ? head : `${head} — ${mappings.join("; ")}`;
    })
    .join("; ");
}

function parseArrayValue(raw: unknown, value: string): unknown[] {
  if (Array.isArray(raw)) {
    return raw;
  }
  if (typeof value === "string" && value.trim().startsWith("[")) {
    try {
      const parsed = JSON.parse(value) as unknown;
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  }
  return [];
}

function formatAdsServerAllowedClients(
  raw: unknown,
  value: string,
  params?: Record<string, unknown>
): string {
  const summary = parseArrayValue(params?.clients_summary, "");
  if (summary.length > 0) {
    return summary.map((client) => str(client)).filter(Boolean).join("; ");
  }

  const clients = parseArrayValue(raw, value);
  if (clients.length === 0) {
    return "None";
  }
  return clients
    .map((client, index) => {
      if (!isRecordValue(client)) {
        return str(client) || `client ${index + 1}`;
      }
      const netId = str(client.ams_net_id) || str(client.net_id) || `client ${index + 1}`;
      const source =
        str(client.source_ip) ||
        str(client.source_cidr);
      if (source) {
        return `${netId} (from ${source})`;
      }
      if (client.unpinned === true) {
        return `${netId} (unpinned lab client)`;
      }
      return netId;
    })
    .filter(Boolean)
    .join("; ");
}

function formatEthercatModules(raw: unknown, value: string): string {
  const modules = parseArrayValue(raw, value);
  if (modules.length === 0) {
    return "None";
  }
  const labels = modules.map((module, index) => {
    if (!isRecordValue(module)) {
      return `module ${index + 1}`;
    }
    const model = str(module.model) || `module ${index + 1}`;
    const slot = str(module.slot);
    const channelCount = Number(module.channels);
    const channels = Number.isFinite(channelCount) ? plural(channelCount, "channel") : "";
    return [model, slot ? `slot ${slot}` : "", channels]
      .filter(Boolean)
      .join(" · ");
  });
  return labels.join("; ");
}

function formatEthercatChannels(raw: unknown, value: string): string {
  const channels = parseArrayValue(raw, value).map((channel) => str(channel)).filter(Boolean);
  if (channels.length === 0) {
    return "None";
  }
  return `${plural(channels.length, "channel")}: ${channels.join(", ")}`;
}

function formatEthercatMockInputs(raw: unknown, value: string): string {
  const frames = parseArrayValue(raw, value);
  return frames.length === 0 ? "None" : plural(frames.length, "mock frame");
}

function formatOnError(value: string): string {
  switch (value) {
    case "fault":
      return "Stop with fault";
    case "warn":
      return "Warn and continue";
    case "ignore":
      return "Ignore";
    default:
      return value;
  }
}

// §0.5 #9 / S-14: render which ST symbol/address each external point becomes ("what do I type after
// adding this?") instead of a bare "N nodes" count. `var` is the ST name; the external ref is
// protocol-specific (node_id / symbol / register / topic / channel).
function pointMapping(point: Record<string, unknown>): string {
  const scalar = (value: unknown): string =>
    value === undefined || value === null ? "" : String(value).trim();
  const stName = scalar(point.var) || scalar(point.address) || scalar(point.name);
  const address = scalar(point.address);
  const external =
    scalar(point.node_id) ||
    scalar(point.symbol) ||
    (point.register !== undefined ? `register ${scalar(point.register)}` : "") ||
    scalar(point.topic) ||
    scalar(point.channel) ||
    scalar(point.tag) ||
    (point.index !== undefined ? `index ${scalar(point.index)}` : "");
  const meta = [scalar(point.type), scalar(point.access) || scalar(point.direction)]
    .filter(Boolean)
    .join(" · ");
  if (!stName && !external) {
    return "";
  }
  let left = stName || "(unnamed)";
  if (address && address !== stName) {
    left += ` (${address})`;
  }
  let result = left;
  if (external) {
    result += ` ← ${external}`;
  }
  if (meta) {
    result += ` · ${meta}`;
  }
  return result;
}

function summaryValueFor(
  protocol: string,
  field: CommFieldSchema,
  value: string,
  raw: unknown,
  params?: Record<string, unknown>
): string {
  if (field.id === "connections") {
    return connectionSummary(raw ?? value);
  }
  if (protocol === "ads_server" && field.id === "clients") {
    return formatAdsServerAllowedClients(raw, value, params);
  }
  if (protocol === "ethercat") {
    if (field.id === "modules") {
      return formatEthercatModules(raw, value);
    }
    if (field.id === "selected_channels") {
      return formatEthercatChannels(raw, value);
    }
    if (field.id === "mock_inputs") {
      return formatEthercatMockInputs(raw, value);
    }
    if (field.id === "timeout_ms" || field.id === "cycle_warn_ms") {
      return value ? `${value} ms` : "";
    }
    if (field.id === "on_error") {
      return formatOnError(value);
    }
  }
  if (field.type === "json_array" && (!value.trim() || value.trim() === "[]")) {
    return "None";
  }
  return value;
}

function endpointStatusRow(health: string, detail: string): string {
  const label = healthLabel(health);
  const withoutConfigFile = normalizedConfiguredDetail(detail);
  if (!withoutConfigFile) {
    return label;
  }
  return `${label} · ${withoutConfigFile}`;
}

const SUMMARY_FIELD_IDS: Record<string, ReadonlySet<string>> = {
  ads_server: new Set([
    "enabled",
    "listen",
    "ams_net_id",
    "ads_port",
    "insecure_transport",
    "writes_enabled",
    "expose",
    "writable",
    "allow_unpinned_clients",
    "clients",
  ]),
  opcua: new Set([
    "enabled",
    "listen",
    "endpoint_path",
    "namespace_uri",
    "expose",
    "security_policy",
    "security_mode",
    "allow_anonymous",
  ]),
};

function includeSummaryField(protocol: string, field: CommFieldSchema): boolean {
  return SUMMARY_FIELD_IDS[protocol]?.has(field.id) ?? true;
}

function summaryLabelFor(protocol: string, field: CommFieldSchema): string {
  if (field.id === "connections") {
    return "Connections";
  }
  if (field.id === "writes_enabled") {
    return "Writes enabled";
  }
  if (/^enable[_\s-]/i.test(field.id) || /^enable\s+/i.test(field.label)) {
    return "Enabled";
  }
  if (/(^|[_\s-])config[_\s-]?path$/i.test(field.id) || /config path/i.test(field.label)) {
    if (protocol === "opcua_client") {
      return "Connection file";
    }
    return "Config file";
  }
  if (/poll[_\s-]?interval/i.test(field.id) || /poll interval/i.test(field.label)) {
    return "Polling";
  }
  return field.label
    .replace(/\bOPC UA client\s+/i, "")
    .replace(/\bOPC UA server\s+/i, "")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^./, (char) => char.toUpperCase());
}

// The compact node band shows the role as terse nomenclature (CLIENT / PUB/SUB). In the roomier
// inspector eyebrow we render it as a calm sentence-case descriptor instead of a shouted all-caps badge.
function roleCap(protocol: string, role: string): string {
  const w = roleWord(protocol, role);
  if (w === "PUB/SUB") return "Pub/Sub";
  if (w === "I/O") return "I/O";
  if (w === "SAME-HOST") return "Same-host";
  return w.charAt(0) + w.slice(1).toLowerCase();
}

const PANEL_STYLE: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 340,
  maxWidth: "92vw",
  background: t.overlay,
  borderLeft: `1px solid ${t.border}`,
  boxShadow: t.shadowOverlay,
  zIndex: 8,
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
};

export function NodeInspector({ node, schema, params, reachable, applyResult, onClose, onFocus, onBrowse, post }: Props) {
  const protocol = str(node.data.protocol);
  const protoSchema = useMemo(
    () => (node.type === "endpoint" ? schema?.protocols.find((p) => p.id === protocol) : undefined),
    [schema, protocol, node.type]
  );
  // §0.5.2: the browse/expose button is now schema-driven — show it only when the backend advertises
  // the capability (comm.schema actions ∋ "browse_symbols") AND the UI has a presentation for it.
  const browse =
    node.type === "endpoint" && protoSchema?.actions.includes("browse_symbols")
      ? browseAction(protocol)
      : undefined;
  const [editing, setEditing] = useState(false);
  // Every node opens on its summary; reset when a different node is selected.
  useEffect(() => setEditing(false), [node.id]);

  // §8 P3b: a runtime node is where per-runtime lifecycle lives. Honest verbs — Start/Stop for the
  // local simulator we own, Connect/Disconnect for a remote we don't (never a fake remote "Stop").
  const isManaged = node.data.managed === true;
  const managedName = isManaged ? str(node.data.managedName) : "";
  const runtimeControls =
    node.type === "runtime"
      ? runtimeNodeControls({
          isLocal: node.id === LOCAL_RUNTIME_NODE_ID,
          health: str(node.data.health),
          attached: node.data.attached === true,
          controlEndpoint: node.data.controlEndpoint
            ? str(node.data.controlEndpoint)
            : undefined,
          managed: isManaged,
          authTokenRequired: isRuntimeAuthTokenFailure(node),
          // Local sim + managed runtimes have logs; remote logs are phase 14 (gated).
          logsAvailable: node.id === LOCAL_RUNTIME_NODE_ID || isManaged,
        })
      : undefined;
  const onControl = (control: RuntimeNodeControl) => {
    switch (control.action) {
      case "runtimeConnect":
        post({
          type: "runtimeConnect",
          endpoint: str(node.data.controlEndpoint),
          label: str(node.data.label),
        });
        return;
      case "runtimeDisconnect":
        post({ type: "runtimeDisconnect" });
        return;
      case "setAuthToken":
        post({ type: "setRuntimeAuthToken", endpoint: str(node.data.controlEndpoint) });
        return;
      case "managedStart":
        post({ type: "runtimeManagedStart", name: managedName });
        return;
      case "managedStop":
        post({ type: "runtimeManagedStop", name: managedName });
        return;
      case "setAsRunTarget":
        post({
          type: "setAsRunTarget",
          endpoint: str(node.data.controlEndpoint),
          isLocal: node.id === LOCAL_RUNTIME_NODE_ID,
          managedName,
        });
        return;
      case "openRuntimeLogs":
        // Managed runtimes have their own logs (fleet runtime logs); the sim uses the debug channel.
        if (managedName) {
          post({ type: "runtimeManagedLogs", name: managedName });
        } else {
          post({ type: "action", action: control.action });
        }
        return;
      case "none":
        return;
      default:
        // Local lifecycle + settings reuse the existing canvas action channel.
        post({ type: "action", action: control.action });
    }
  };

  if (editing && protoSchema) {
    return (
      <EditableEndpoint
        node={node}
        protoSchema={protoSchema}
        params={params}
        reachable={reachable}
        applyResult={applyResult}
        onBack={() => setEditing(false)}
        onClose={onClose}
        post={post}
      />
    );
  }

  return (
      <SummaryView
        node={node}
        protoSchema={protoSchema}
        params={params}
        applyResult={applyResult}
        onEdit={protoSchema ? () => setEditing(true) : undefined}
        onTest={
          node.type === "endpoint" && protoSchema?.supports_test && reachable
            ? () => post({ type: "commTest", protocol, params: testParamsFor(protocol, params), target: node.id })
            : undefined
        }
        onBrowse={onBrowse && browse ? () => onBrowse(node) : undefined}
        browseLabel={browse?.label}
        runtimeControls={runtimeControls}
      onControl={onControl}
      onClose={onClose}
      onFocus={onFocus}
    />
  );
}

// ---- read-only summary (default view for every node) ----
function SummaryView({
  node,
  protoSchema,
  params,
  applyResult,
  onEdit,
  onTest,
  onBrowse,
  browseLabel,
  runtimeControls,
  onControl,
  onClose,
  onFocus,
}: {
  node: InspectorNode;
  protoSchema?: CommProtocolSchema;
  params?: Record<string, unknown>;
  applyResult?: CommApplyResponse;
  onEdit?: () => void;
  onTest?: () => void;
  onBrowse?: () => void;
  browseLabel?: string;
  runtimeControls?: RuntimeNodeControl[];
  onControl?: (control: RuntimeNodeControl) => void;
  onClose: () => void;
  onFocus: (id: string) => void;
}) {
  const d = node.data;
  const protocol = str(d.protocol);
  const resultApplies = applyResult?.protocol === protocol;
  const ok = resultApplies && (applyResult?.applied || applyResult?.lifecycle_effect === "test_ok");
  const blocked = resultApplies && applyResult?.lifecycle_effect === "blocked";

  const [showAllActions, setShowAllActions] = useState(false);
  // S-14: a node inspector shows at most TWO visible secondary actions; any extras collapse behind an
  // overflow disclosure so a node never becomes a toolbar full of buttons.
  const runtimeControlLayout = runtimeNodeControlLayout(
    runtimeControls,
    onControl,
    [{ key: "focus", label: "Focus", enabled: true, onClick: () => onFocus(node.id) }],
    showAllActions
  );

  let title: string;
  let kindLabel: string;
  let accent: string | undefined;
  let health = "";
  const rows: Array<[string, string]> = [];

  if (protoSchema) {
    // Endpoint with a known protocol: show its current settings (read-only).
    title = protocolName(protocol);
    kindLabel = `${roleCap(protocol, str(d.role))} · ${str(d.kind) === "field" ? "device" : "endpoint"}`;
    accent = protocolColor(protocol);
    health = str(d.health);
    rows.push(["Name", str(d.name)]);
    const values = valuesFor(protoSchema, params);
    for (const field of protoSchema.fields) {
      if (!includeSummaryField(protocol, field)) {
        continue;
      }
      const v = field.secret
        ? (values[field.id] ? "••• (set)" : "—")
        : summaryValueFor(protocol, field, values[field.id], params?.[field.id], params);
      if (v) {
        rows.push([summaryLabelFor(protocol, field), v]);
      }
    }
    if (d.detail) {
      rows.push(["State", endpointStatusRow(health, str(d.detail))]);
    }
  } else {
    switch (node.type) {
      case "runtime":
        title = str(d.label);
        kindLabel = "Runtime";
        health = str(d.health);
        rows.push(["State", stateSummary(health, str(d.detail))]);
        {
          const mode = runtimeModeLabel(str(d.mode));
          if (mode) {
            rows.push(["Mode", mode]);
          }
        }
        rows.push(["Endpoints", str(d.endpointCount)]);
        break;
      case "host":
        title = str(d.label);
        kindLabel = "Host";
        health = str(d.health);
        rows.push(["Address", str(d.sub)], ["State", healthLabel(health)], ["Runtimes", str(d.runtimeCount)], ["Endpoints", str(d.endpointCount)]);
        break;
      case "container":
        title = str(d.label);
        kindLabel = "Container";
        rows.push(["Image", str(d.image)], ["State", str(d.status)]);
        break;
      case "external":
        title = str(d.label);
        kindLabel = "External system";
        rows.push(["Presents", str(d.sub)], ["Scope", "external · configured on our side"]);
        break;
      case "endpoint":
        // Endpoint without a loaded schema: still show its basic facts (never blank).
        title = str(d.name) || protocolName(protocol);
        kindLabel = `${roleCap(protocol, str(d.role))} · endpoint`;
        health = str(d.health);
        rows.push(
          ["Protocol", protocolName(protocol)],
          ["Role", roleWord(protocol, str(d.role))],
          ["State", endpointStatusRow(health, str(d.detail))]
        );
        break;
      default:
        title = str(d.label) || str(d.name) || node.id;
        kindLabel = str(node.type) || "node";
    }
  }
  const shown = rows.filter(([, v]) => v);

  return (
    <aside className="trust-inspector" style={PANEL_STYLE} aria-label="Node summary">
      <header className="trust-inspector__header">
        {accent && <span style={{ flex: "none", width: 10, height: 10, borderRadius: 3, background: accent }} />}
        <div style={{ flex: 1, minWidth: 0 }}>
          <span className="trust-inspector__eyebrow" style={{ display: "block", marginBottom: 2 }}>Devices & Connections / {kindLabel}</span>
          <strong className="trust-inspector__title" style={{ display: "block", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{title}</strong>
        </div>
        {health && (
          <span title={health} style={{ flex: "none", width: 10, height: 10, borderRadius: "50%", background: healthColor(health), boxShadow: `0 0 0 2px ${tint(healthColor(health), 0.18)}` }} />
        )}
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </header>
      <div className="trust-section trust-section--grow" style={{ paddingBottom: 18 }}>
        {shown.length === 0 ? (
          <p className="trust-empty" style={{ padding: 0, textAlign: "left" }}>No further details.</p>
        ) : (
          shown.map(([k, v]) => (
            <div key={k} style={{ display: "grid", gridTemplateColumns: "132px 1fr", gap: 10, fontSize: 12, lineHeight: 1.55, marginBottom: 7 }}>
              <span style={{ color: t.textMuted, overflowWrap: "anywhere" }}>{k}</span>
              <span style={{ color: t.text, overflowWrap: "anywhere" }}>{v}</span>
            </div>
          ))
        )}
      </div>
      {/* Pinned between the scroll body and the footer so the result message is never
          hidden behind the footer buttons. */}
      {resultApplies && applyResult && (applyResult.message || ok || blocked) && (
        <div
          className={`trust-message ${ok ? "trust-message--ok" : blocked ? "trust-message--error" : ""}`}
          style={{ margin: "0 14px 10px" }}
        >
          {applyResult.message || (ok ? "Test passed." : "")}
        </div>
      )}
      <footer className="trust-section" style={{ display: "flex", flexWrap: "wrap", gap: 8, borderBottom: "none", borderTop: `1px solid ${t.border}`, background: t.surface }}>
        {runtimeControls && onControl ? (
          <>
            {runtimeControlLayout.primary && (
              <button
                key={`${runtimeControlLayout.primary.action}:${runtimeControlLayout.primary.label}`}
                onClick={() => onControl(runtimeControlLayout.primary!)}
                disabled={!runtimeControlLayout.primary.enabled}
                className="trust-button trust-button--primary"
                style={{ flexBasis: "100%" }}
              >
                {runtimeControlLayout.primary.label}
              </button>
            )}
            {runtimeControlLayout.visibleSecondary.map((item) => (
              <button
                key={item.key}
                onClick={item.onClick}
                disabled={!item.enabled}
                className="trust-button"
                style={{ flex: 1 }}
              >
                {item.label}
              </button>
            ))}
            {runtimeControlLayout.hasOverflow && (
              <button
                onClick={() => setShowAllActions((value) => !value)}
                className="trust-button"
                style={{ flex: 1 }}
                aria-label={showAllActions ? "Show fewer actions" : "More actions"}
                title={showAllActions ? "Show fewer actions" : "More actions"}
              >
                {showAllActions ? "Less" : "⋯"}
              </button>
            )}
          </>
        ) : (
          <>
            {onEdit && (
              <button onClick={onEdit} className="trust-button trust-button--primary" style={{ flex: 1 }}>Edit settings</button>
            )}
            {onTest && (
              <button onClick={onTest} className="trust-button">Test</button>
            )}
            {onBrowse && (
              <button onClick={onBrowse} className="trust-button">{browseLabel ?? "Browse"}</button>
            )}
            <button onClick={() => onFocus(node.id)} className="trust-button" style={onEdit || onTest || onBrowse ? undefined : { flex: 1 }}>Focus</button>
          </>
        )}
      </footer>
    </aside>
  );
}

// ---- editable form (after pressing Edit) ----
function EditableEndpoint({
  node,
  protoSchema,
  params,
  reachable,
  applyResult,
  onBack,
  onClose,
  post,
}: {
  node: InspectorNode;
  protoSchema: CommProtocolSchema;
  params?: Record<string, unknown>;
  reachable: boolean;
  applyResult?: CommApplyResponse;
  onBack: () => void;
  onClose: () => void;
  post: (message: unknown) => void;
}) {
  const protocol = str(node.data.protocol);
  const health = str(node.data.health);
  const [values, setValues] = useState<Record<string, string>>(() => valuesFor(protoSchema, params));
  const [confirmRemove, setConfirmRemove] = useState(false);
  const paramsKey = JSON.stringify(params ?? {});
  const schemaKey = `${protoSchema.id}:${protoSchema.fields.map((field) => field.id).join("|")}`;
  useEffect(() => {
    setValues(valuesFor(protoSchema, params));
  }, [node.id, schemaKey, paramsKey]);
  useEffect(() => {
    setConfirmRemove(false);
  }, [node.id]);

  const fieldErrors = new Map((applyResult?.field_errors ?? []).map((e) => [e.field, e.message]));
  const isDisabled = health === "disabled";
  const canDisable = protoSchema.actions.includes("disable") && !isDisabled;
  const send = (type: string, extra?: Record<string, unknown>) => {
    if (type !== "commRemove") {
      setConfirmRemove(false);
    }
    post({ type, protocol, params: buildParams(protoSchema, values), target: node.id, ...extra });
  };
  const requestRemove = () => {
    if (!confirmRemove) {
      setConfirmRemove(true);
      return;
    }
    send("commRemove");
  };

  const ok = applyResult && (applyResult.applied || applyResult.lifecycle_effect === "test_ok");
  const blocked = applyResult && applyResult.lifecycle_effect === "blocked";

  return (
    <aside className="trust-inspector" style={PANEL_STYLE} aria-label="Node settings">
      <header className="trust-inspector__header">
        <button onClick={onBack} aria-label="Back" title="Back to summary" style={iconBtn}>‹</button>
        <span style={{ flex: "none", width: 10, height: 10, borderRadius: 3, background: protocolColor(protocol) }} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__eyebrow">Devices & Connections / {roleWord(protocol, str(node.data.role))} edit</div>
          <div className="trust-inspector__title">{protocolName(protocol)}</div>
        </div>
        {health && (
          <span title={health} style={{ flex: "none", width: 10, height: 10, borderRadius: "50%", background: healthColor(health), boxShadow: `0 0 0 2px ${tint(healthColor(health), 0.18)}` }} />
        )}
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </header>

      <div className="trust-section trust-section--grow">
        {protoSchema.purpose && (
          <p className="trust-help" style={{ marginBottom: 14 }}>{protoSchema.purpose}</p>
        )}
        {protoSchema.fields.map((field) => (
          <Field
            key={field.id}
            field={field}
            value={values[field.id] ?? ""}
            error={fieldErrors.get(field.id)}
            onChange={(v) => setValues((prev) => ({ ...prev, [field.id]: v }))}
          />
        ))}
      </div>

      {/* Pinned between the scroll body and the footer so the save/validation result and
          the disabled note are never hidden behind the footer buttons. */}
      {applyResult && (applyResult.message || ok || blocked) && (
        <div
          className={`trust-message ${ok ? "trust-message--ok" : blocked ? "trust-message--error" : ""}`}
          style={{ margin: "0 14px 10px" }}
        >
          {applyResult.message || (ok ? "Saved." : "")}
        </div>
      )}
      {isDisabled && !applyResult && (
        <div className="trust-message" style={{ margin: "0 14px 10px" }}>
          This endpoint is disabled. Use Enable to turn it back on; restart the runtime to apply the change.
        </div>
      )}

      <footer className="trust-section" style={{ display: "flex", flexWrap: "wrap", gap: 8, borderBottom: "none" }}>
        {confirmRemove ? (
          <>
            <div
              className="trust-message trust-message--error endpoint-remove-confirmation"
              style={{ flexBasis: "100%", margin: 0 }}
            >
              Remove this endpoint from the project? This writes the config file and takes effect after restart.
            </div>
            <button onClick={() => setConfirmRemove(false)} className="trust-button" style={{ flex: 1 }}>
              Cancel
            </button>
            <button onClick={requestRemove} className="trust-button trust-button--danger" style={{ flex: 1 }}>
              Confirm remove
            </button>
          </>
        ) : (
          <>
            <button onClick={() => send("commSave", { action: "upsert" })} className="trust-button trust-button--primary" style={{ flex: 1 }}>{isDisabled ? "Enable" : "Save"}</button>
            <button onClick={onClose} className="trust-button">Cancel</button>
            {protoSchema.supports_test && reachable && (
              <button onClick={() => send("commTest")} className="trust-button">Test</button>
            )}
            {canDisable && (
              <button onClick={() => send("commDisable", { action: "disable" })} className="trust-button">
                Disable
              </button>
            )}
            <button onClick={requestRemove} className="trust-button trust-button--danger">
              Remove
            </button>
            {reachable && (
              <button onClick={() => send("commApplyLive", { action: "upsert" })} title="Push this config to the running runtime now" className="trust-button" style={{ flexBasis: "100%" }}>
                Apply to running runtime
              </button>
            )}
          </>
        )}
      </footer>
    </aside>
  );
}

const iconBtn: React.CSSProperties = { border: "none", background: "transparent", color: t.textMuted, fontSize: 14, cursor: "pointer" };
