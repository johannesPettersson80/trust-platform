import React, { useEffect, useMemo, useState } from "react";
import { healthColor, protocolColor, protocolName, roleWord } from "./nodes";
import { t, tint } from "./theme";
import { buildParams, Field, valuesFor } from "./SchemaFields";
import { browseAction } from "./browseActions";
import {
  runtimeNodeControls,
  type RuntimeNodeControl,
} from "./runtimeNodeControls";
import { LOCAL_RUNTIME_NODE_ID } from "./types";
import type {
  CommApplyResponse,
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
          // Local sim + managed runtimes have logs; remote logs are phase 14 (gated).
          logsAvailable: node.id === LOCAL_RUNTIME_NODE_ID || isManaged,
        })
      : undefined;
  const onControl = (control: RuntimeNodeControl) => {
    switch (control.action) {
      case "runtimeConnect":
        post({ type: "runtimeConnect", endpoint: str(node.data.controlEndpoint) });
        return;
      case "runtimeDisconnect":
        post({ type: "runtimeDisconnect" });
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
      onEdit={protoSchema ? () => setEditing(true) : undefined}
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
  onEdit,
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
  onEdit?: () => void;
  onBrowse?: () => void;
  browseLabel?: string;
  runtimeControls?: RuntimeNodeControl[];
  onControl?: (control: RuntimeNodeControl) => void;
  onClose: () => void;
  onFocus: (id: string) => void;
}) {
  const d = node.data;
  const protocol = str(d.protocol);

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
    rows.push(["name", str(d.name)]);
    const values = valuesFor(protoSchema, params);
    for (const field of protoSchema.fields) {
      const v = field.secret ? (values[field.id] ? "••• (set)" : "—") : values[field.id];
      if (v) {
        rows.push([field.label.toLowerCase(), v]);
      }
    }
    if (d.detail) {
      rows.push(["status", `${healthLabel(health)} · ${str(d.detail)}`]);
    }
  } else {
    switch (node.type) {
      case "runtime":
        title = str(d.label);
        kindLabel = "Runtime";
        health = str(d.health);
        rows.push(["mode", str(d.mode)], ["status", healthLabel(health)], ["endpoints", str(d.endpointCount)], ["detail", str(d.detail)]);
        break;
      case "host":
        title = str(d.label);
        kindLabel = "Host";
        health = str(d.health);
        rows.push(["address", str(d.sub)], ["status", healthLabel(health)], ["runtimes", str(d.runtimeCount)], ["endpoints", str(d.endpointCount)]);
        break;
      case "container":
        title = str(d.label);
        kindLabel = "Container";
        rows.push(["image", str(d.image)], ["status", str(d.status)]);
        break;
      case "external":
        title = str(d.label);
        kindLabel = "External system";
        rows.push(["presents", str(d.sub)], ["scope", "external · configured on our side"]);
        break;
      case "endpoint":
        // Endpoint without a loaded schema: still show its basic facts (never blank).
        title = str(d.name) || protocolName(protocol);
        kindLabel = `${roleCap(protocol, str(d.role))} · endpoint`;
        health = str(d.health);
        rows.push(
          ["protocol", protocolName(protocol)],
          ["role", roleWord(protocol, str(d.role))],
          ["status", healthLabel(health)],
          ["detail", str(d.detail)]
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
          <strong className="trust-inspector__title" style={{ display: "block", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{title}</strong>
          <span className="trust-inspector__eyebrow" style={{ display: "block", marginTop: 2 }}>{kindLabel}</span>
        </div>
        {health && (
          <span title={health} style={{ flex: "none", width: 10, height: 10, borderRadius: "50%", background: healthColor(health), boxShadow: `0 0 0 2px ${tint(healthColor(health), 0.18)}` }} />
        )}
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </header>
      <div className="trust-section trust-section--grow">
        {shown.length === 0 ? (
          <p className="trust-empty" style={{ padding: 0, textAlign: "left" }}>No further details.</p>
        ) : (
          shown.map(([k, v]) => (
            <div key={k} style={{ display: "flex", gap: 10, fontSize: 12, lineHeight: 1.55, marginBottom: 7 }}>
              <span style={{ color: t.textMuted, flex: "none", minWidth: 84 }}>{k}</span>
              <span style={{ color: t.text, overflowWrap: "anywhere" }}>{v}</span>
            </div>
          ))
        )}
      </div>
      <footer className="trust-section" style={{ display: "flex", flexWrap: "wrap", gap: 8, borderBottom: "none" }}>
        {runtimeControls && onControl ? (
          <>
            {runtimeControls.map((control) => (
              <button
                key={`${control.action}:${control.label}`}
                onClick={() => onControl(control)}
                disabled={!control.enabled}
                className={`trust-button${control.kind === "primary" ? " trust-button--primary" : ""}`}
                style={control.kind === "primary" ? { flexBasis: "100%" } : { flex: 1 }}
              >
                {control.label}
              </button>
            ))}
            <button onClick={() => onFocus(node.id)} className="trust-button" style={{ flex: 1 }}>Focus</button>
          </>
        ) : (
          <>
            {onEdit && (
              <button onClick={onEdit} className="trust-button trust-button--primary" style={{ flex: 1 }}>Edit settings</button>
            )}
            {onBrowse && (
              <button onClick={onBrowse} className="trust-button">{browseLabel ?? "Browse"}</button>
            )}
            <button onClick={() => onFocus(node.id)} className="trust-button" style={onEdit || onBrowse ? undefined : { flex: 1 }}>Focus</button>
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
  useEffect(() => {
    setValues(valuesFor(protoSchema, params));
  }, [node.id, protoSchema, params]);

  const fieldErrors = new Map((applyResult?.field_errors ?? []).map((e) => [e.field, e.message]));
  const send = (type: string, extra?: Record<string, unknown>) =>
    post({ type, protocol, params: buildParams(protoSchema, values), target: node.id, ...extra });

  const ok = applyResult && (applyResult.applied || applyResult.lifecycle_effect === "test_ok");
  const blocked = applyResult && applyResult.lifecycle_effect === "blocked";

  return (
    <aside className="trust-inspector" style={PANEL_STYLE} aria-label="Node settings">
      <header className="trust-inspector__header">
        <button onClick={onBack} aria-label="Back" title="Back to summary" style={iconBtn}>‹</button>
        <span style={{ flex: "none", width: 10, height: 10, borderRadius: 3, background: protocolColor(protocol) }} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__title">{protocolName(protocol)}</div>
          <div className="trust-inspector__eyebrow" style={{ marginTop: 2 }}>
            {roleWord(protocol, str(node.data.role))} · edit
          </div>
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
        {applyResult && (applyResult.message || ok || blocked) && (
          <div
            className={`trust-message ${ok ? "trust-message--ok" : blocked ? "trust-message--error" : ""}`}
          >
            {applyResult.message || (ok ? "Saved." : "")}
          </div>
        )}
      </div>

      <footer className="trust-section" style={{ display: "flex", flexWrap: "wrap", gap: 8, borderBottom: "none" }}>
        <button onClick={() => send("commSave", { action: "upsert" })} className="trust-button trust-button--primary" style={{ flex: 1 }}>Save</button>
        {protoSchema.supports_test && reachable && (
          <button onClick={() => send("commTest")} className="trust-button">Test</button>
        )}
        <button onClick={() => send("commRemove")} className="trust-button trust-button--danger">Remove</button>
        {reachable && (
          <button onClick={() => send("commApplyLive", { action: "upsert" })} title="Push this config to the running runtime now" className="trust-button" style={{ flexBasis: "100%" }}>
            Apply to running runtime
          </button>
        )}
      </footer>
    </aside>
  );
}

const iconBtn: React.CSSProperties = { border: "none", background: "transparent", color: t.textMuted, fontSize: 14, cursor: "pointer" };
