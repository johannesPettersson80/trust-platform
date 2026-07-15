import React, { memo, useState } from "react";
import { Handle, NodeToolbar, Position, type NodeProps } from "@xyflow/react";
import { BusNode } from "./BusNode";
import {
  connectorConnectionLabel,
  connectorHealthLabel,
  connectorSignalsSummary,
  discoveryConfidenceLabel,
} from "./connectorPresentation";
import { useEditMode, type AddSlotRequest } from "./editMode";
import { protocolBadgeLabel, protocolColor, protocolName } from "./protocolMeta";
import { t, tint } from "./theme";
import {
  LOCAL_RUNTIME_NODE_ID,
  type ContainerNodeData,
  type EndpointNodeData,
  type ExternalNodeData,
  type HostNodeData,
  type RuntimeNodeData,
  type SlotNodeData,
} from "./types";

// Inline SVG icons (emojis render as tofu squares in the webview).
const svgProps = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.7,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};
const ICONS = {
  host: (
    <svg width="17" height="17" viewBox="0 0 24 24" {...svgProps}>
      <rect x="3" y="4.5" width="18" height="6" rx="1.5" />
      <rect x="3" y="13.5" width="18" height="6" rx="1.5" />
      <circle cx="6.6" cy="7.5" r="1" fill="currentColor" stroke="none" />
      <circle cx="6.6" cy="16.5" r="1" fill="currentColor" stroke="none" />
    </svg>
  ),
  runtime: (
    <svg width="15" height="15" viewBox="0 0 24 24" {...svgProps}>
      <rect x="6.5" y="6.5" width="11" height="11" rx="1.5" />
      <path d="M9.5 3.5v3M14.5 3.5v3M9.5 17.5v3M14.5 17.5v3M3.5 9.5h3M3.5 14.5h3M17.5 9.5h3M17.5 14.5h3" />
    </svg>
  ),
  container: (
    <svg width="14" height="14" viewBox="0 0 24 24" {...svgProps}>
      <path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z" />
      <path d="M4 7.5L12 12l8-4.5M12 12v9" />
    </svg>
  ),
  external: (
    <svg width="14" height="14" viewBox="0 0 24 24" {...svgProps}>
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18M12 3c2.6 2.4 4 5.6 4 9s-1.4 6.6-4 9c-2.6-2.4-4-5.6-4-9s1.4-6.6 4-9z" />
    </svg>
  ),
};

// Status → theme colour. Maps onto VS Code chart/status tokens (see theme.ts) so it tracks the
// user's theme; colour is always paired with a label (mode badge, hover card, inspector) for
// colour-blind safety.
export function healthColor(health: string): string {
  switch (health) {
    case "connected":
      return t.ok;
    case "degraded":
      return t.warn;
    case "error":
    case "runtime_unreachable":
      return t.danger;
    default:
      return t.idle; // not_configured / configured_policy / pending / simulate / unknown
  }
}

// The role band text (uppercase). Local I/O has no remote role → "I/O".
export function roleWord(protocol: string, role: string): string {
  if (role === "draft") {
    return "DRAFT";
  }
  switch (protocol) {
    case "ethercat":
      return "MASTER";
    case "modbus_tcp":
    case "modbus":
      return "CLIENT";
    case "opcua":
      return role === "client" ? "CLIENT" : "SERVER";
    case "ads":
      return role === "server" ? "SERVER" : "CLIENT";
    case "ads_server":
      return "SERVER";
    case "mqtt":
      return "PUB/SUB";
    case "mesh":
      return "PEER";
    case "discovery":
      return "ADVERTISE";
    case "realtime":
    case "realtime_t0":
      return "SAME-HOST";
    case "runtime_cloud":
    case "federation":
      return "POLICY";
    case "web":
      return "SERVER";
    case "gpio":
    case "simulated":
    case "loopback":
      return "I/O";
    default:
      return (role || "").toUpperCase();
  }
}

// Ports are invisible plumbing: wires still anchor to their position, but the nub never shows — so a
// runtime's (rarely used) left/right link ports don't read as dots that connect to nothing. Devices
// are added via the Edit-mode "+" slots, not by dragging from a handle, so nothing needs a visible port.
const PORT_STYLE: React.CSSProperties = {
  background: "transparent",
  width: 6,
  height: 6,
  border: "none",
  opacity: 0,
  minWidth: 0,
  minHeight: 0,
};

// §7 honesty: unproven/pending config renders ghost/dashed and never green.
const PENDING_STATES = new Set(["pending", "not_configured", "not_in_build", "configured_policy", "unknown", "disabled"]);
function isPending(health: string): boolean {
  return PENDING_STATES.has(health);
}

const ADVANCED_PROTOCOLS = new Set(["mesh", "openot", "realtime", "realtime_t0", "runtime_cloud", "federation"]);
function isDraftLikeEndpoint(protocol: string, role: string, health: string): boolean {
  return (
    role === "draft" ||
    (ADVANCED_PROTOCOLS.has(protocol) &&
      ["pending", "configured_policy", "not_configured", "unknown"].includes(health))
  );
}

// A HOST is a machine (where), not a process. Its lifecycle state ("pending"/"stopped"/"connected") is
// really the RUNTIME's connection state — showing it on the host conflates "the runtime isn't connected"
// with "the machine is pending" (e.g. the local Pi we're literally running on showing "Pending"). So the
// host only surfaces a status pill when something is wrong with reaching THAT machine; otherwise the host
// is just its name and the runtime node carries the lifecycle status.
const HOST_PROBLEM_STATES = new Set(["error", "degraded", "runtime_unreachable"]);
function isHostProblem(health: string): boolean {
  return HOST_PROBLEM_STATES.has(health);
}

// A node card surface. Hairline border + soft role tint (theme-aware); dashed + dimmed when pending.
function cardStyle(
  health: string,
  opts: { background?: string; border?: string; radius?: number } = {}
): React.CSSProperties {
  const pending = isPending(health);
  return {
    width: "100%",
    height: "100%",
    border: `1px ${pending ? "dashed" : "solid"} ${opts.border ?? t.border}`,
    borderRadius: opts.radius ?? t.radius,
    background: opts.background ?? t.surface,
    boxShadow: pending ? "none" : t.shadow,
    opacity: pending ? 0.9 : 1,
    transition: `box-shadow ${t.ease}, border-color ${t.ease}, opacity ${t.ease}`,
  };
}

// Honest status word — paired with the dot so status never relies on colour alone (accessibility) and
// is one clear signal instead of a colour-dot plus a separate state badge.
function statusLabel(health: string): string {
  switch (health) {
    case "connected":
      return "Online";
    case "degraded":
      return "Degraded";
    case "error":
      return "Error";
    case "runtime_unreachable":
      return "Unreachable";
    case "pending":
      // Configured but not running / state not yet known — honest-neutral, never overclaim a live connect.
      return "Pending";
    case "configured_policy":
      return "Configured only";
    case "stopped":
      return "Stopped";
    case "disabled":
      return "Disabled";
    case "not_configured":
      return "Not set up";
    case "simulate":
      return "Simulator";
    case "unknown":
      return "Unknown";
    default:
      return health ? health.charAt(0).toUpperCase() + health.slice(1) : "Unknown";
  }
}

// One status signal for a host/runtime: a quiet pill with a status-coloured dot + the state word.
// Replaces the old "mode badge that showed the state" + a separate dot (which said the same thing twice).
function StatusPill({ health, label, tone }: { health: string; label?: string; tone?: string }) {
  const c = tone ?? healthColor(health);
  const live = !tone && health === "connected";
  return (
    <span
      title={health}
      style={{
        flex: "none",
        display: "inline-flex",
        alignItems: "center",
        gap: 5,
        fontSize: 10,
        fontWeight: 600,
        color: t.textMuted,
        background: t.canvas,
        border: `1px solid ${t.border}`,
        borderRadius: t.pill,
        padding: "2px 8px 2px 6px",
        whiteSpace: "nowrap",
      }}
    >
      <span
        className={live ? "trust-dot trust-dot--live" : "trust-dot"}
        style={{ width: 7, height: 7, borderRadius: "50%", background: c, boxShadow: `0 0 0 3px ${tint(c, 0.16)}` }}
      />
      {label ?? statusLabel(health)}
    </span>
  );
}

// Hover-card: the "see more" details that don't belong on the minimal node body.
function HoverCard({ title, rows }: { title: string; rows: Array<[string, string]> }) {
  return (
    <div
      style={{
        minWidth: 180,
        maxWidth: 280,
        textAlign: "left",
        background: t.overlay,
        border: `1px solid ${t.border}`,
        borderRadius: t.radiusLg,
        boxShadow: t.shadowOverlay,
        padding: "9px 11px",
      }}
    >
      <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6, color: t.text }}>{title}</div>
      {rows
        .filter(([, v]) => v)
        .map(([k, v]) => (
          <div key={k} style={{ display: "flex", gap: 10, fontSize: 11, lineHeight: 1.5 }}>
            <span style={{ color: t.textMuted, flex: "none", minWidth: 62 }}>{k}</span>
            <span style={{ color: t.text, overflowWrap: "anywhere" }}>{v}</span>
          </div>
        ))}
    </div>
  );
}

export const HostNode = memo(({ data }: NodeProps) => {
  const d = data as HostNodeData;
  const [hover, setHover] = useState(false);
  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={cardStyle(d.health, {
        background: t.roleHostBg,
        border: t.roleHostBorder,
        radius: t.radiusLg,
      })}
    >
      <NodeToolbar isVisible={hover} position={Position.Top}>
        <HoverCard
          title={d.label}
          rows={[["detail", d.sub], ["health", d.health], ["runtimes", String(d.runtimeCount)], ["endpoints", String(d.endpointCount)]]}
        />
      </NodeToolbar>
      {/* Title over status: hostname always shows in full; reachability sits on its own row. */}
      <div style={{ display: "flex", flexDirection: "column", gap: 6, padding: "9px 12px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <span style={{ flex: "none", display: "flex", color: t.textMuted }}>{ICONS.host}</span>
          <strong title={d.label} style={{ flex: 1, minWidth: 0, fontSize: 13.5, fontWeight: 600, color: t.text, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
            {d.label}
          </strong>
        </div>
        <div style={{ display: "flex" }}>
          {/* Unreachable is a "can't reach this machine" warning (amber), not a hard error (red). */}
          <StatusPill
            health={d.health}
            label={isHostProblem(d.health) ? "Unreachable" : "Reachable"}
            tone={isHostProblem(d.health) ? t.warn : undefined}
          />
        </div>
      </div>
    </div>
  );
});
HostNode.displayName = "HostNode";

export const ContainerNode = memo(({ data }: NodeProps) => {
  const d = data as ContainerNodeData;
  const [hover, setHover] = useState(false);
  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{ width: "100%", height: "100%", border: `1px dashed ${t.borderSubtle}`, borderRadius: t.radius, background: "transparent" }}
    >
      <NodeToolbar isVisible={hover} position={Position.Top}>
        <HoverCard title={d.label} rows={[["image", d.image], ["status", d.status]]} />
      </NodeToolbar>
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 11px", color: t.text }}>
        <span style={{ display: "flex", color: t.textMuted }}>{ICONS.container}</span>
        <strong style={{ flex: 1, minWidth: 0, fontSize: 12, fontWeight: 600, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{d.label}</strong>
        <span style={{ flex: "none", color: t.textMuted, fontSize: 10 }}>{d.status}</span>
      </div>
    </div>
  );
});
ContainerNode.displayName = "ContainerNode";

export const RuntimeNode = memo(({ id, data }: NodeProps) => {
  const d = data as RuntimeNodeData;
  const [hover, setHover] = useState(false);
  const { editMode } = useEditMode();
  // First-run orientation: a runtime with no devices is otherwise a blank box. The primary path is the
  // toolbar + Add button; Edit-mode slots remain a topology-placement affordance.
  // Honesty gate: only assert "no devices" for the local simulator (an unambiguous fresh start) or a
  // runtime we're actually connected to. A merely-stopped managed/remote runtime may have devices we
  // just can't see yet (they surface on connect), so we must NOT claim it's empty.
  const showEmpty =
    !editMode &&
    d.endpointCount === 0 &&
    (id === LOCAL_RUNTIME_NODE_ID || d.health === "connected");
  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        ...cardStyle(d.health, {
          background: t.roleRuntimeBg,
          border: t.roleRuntimeBorder,
        }),
        display: "flex",
        flexDirection: "column",
      }}
    >
      <NodeToolbar isVisible={hover} position={Position.Top}>
        <HoverCard
          title={d.label}
          rows={[["mode", d.mode], ["health", d.health], ["endpoints", String(d.endpointCount)], ["container", d.container ?? ""], ["detail", d.detail]]}
        />
      </NodeToolbar>
      <Handle type="target" position={Position.Left} style={PORT_STYLE} />
      <Handle type="source" position={Position.Right} style={PORT_STYLE} />
      {/* Title over status: the name gets the full width (no truncation), status sits on its own row. */}
      <div style={{ display: "flex", flexDirection: "column", gap: 6, padding: "9px 12px" }}>
        <div style={{ display: "flex", alignItems: "flex-start", gap: 9 }}>
          <span style={{ flex: "none", display: "flex", color: t.textMuted, marginTop: 1 }}>{ICONS.runtime}</span>
          <strong
            title={d.label}
            style={{
              flex: 1,
              minWidth: 0,
              fontSize: 13,
              fontWeight: 600,
              color: t.text,
              lineHeight: 1.25,
              display: "-webkit-box",
              WebkitLineClamp: 2,
              WebkitBoxOrient: "vertical",
              overflow: "hidden",
              overflowWrap: "anywhere",
            }}
          >
            {d.label}
          </strong>
          {d.container && (
            <span title={`container: ${d.container}`} style={{ flex: "none", display: "inline-flex", alignItems: "center", color: t.textMuted, border: `1px solid ${t.border}`, borderRadius: t.radiusSm, padding: "2px 5px" }}>
              {ICONS.container}
            </span>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
          <StatusPill health={d.health} />
          {d.runTarget === true && (
            <span
              title="Selected run target"
              style={{
                display: "inline-flex",
                alignItems: "center",
                border: `1px solid ${t.accent}`,
                borderRadius: t.pill,
                background: t.selectedBg,
                color: t.text,
                fontSize: 10.5,
                fontWeight: 650,
                lineHeight: 1,
                padding: "3px 7px",
              }}
            >
              Run target
            </span>
          )}
        </div>
      </div>
      {showEmpty && (
        <div style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 5, padding: "4px 16px 14px", textAlign: "center" }}>
          <div style={{ fontSize: 11.5, fontWeight: 600, color: t.textMuted }}>No devices yet</div>
          <div style={{ fontSize: 10.5, color: t.textSubtle, lineHeight: 1.5 }}>
            Use <span style={{ color: t.textMuted, fontWeight: 600 }}>+ Add</span> to add one, or <span style={{ color: t.textMuted, fontWeight: 600 }}>Discover</span> to scan the network.
          </div>
        </div>
      )}
    </div>
  );
});
RuntimeNode.displayName = "RuntimeNode";

function connectorRows(d: EndpointNodeData): Array<[string, string]> {
  if (!d.connector) {
    return [];
  }
  return [
    ["Connection", connectorConnectionLabel(d.connector.state)],
    ["Health", connectorHealthLabel(d.connector.health)],
    ["Verification", discoveryConfidenceLabel(d.connector.confidence)],
    ["Signals", connectorSignalsSummary(d.connector.point_counts)],
  ];
}

// Layout (app-icon style): protocol name on top, role in a coloured band below.
export const EndpointNode = memo(({ data }: NodeProps) => {
  const d = data as EndpointNodeData;
  const draftLike = isDraftLikeEndpoint(d.protocol, d.role, d.health);
  const pc = draftLike ? t.protocolMuted : protocolColor(d.protocol);
  const statusTone = draftLike ? t.protocolMuted : healthColor(d.health);
  const [hover, setHover] = useState(false);
  // §0.2: everything networked gets a wire/port; only local I/O does not.
  const isComm = !["gpio", "simulated", "loopback"].includes(d.protocol);
  // §10.2: EtherCAT segment slaves render as compact child rows inside the node (containment, no wires).
  const slaves = d.protocol === "ethercat" ? d.children ?? [] : [];
  const hasSlaves = slaves.length > 0;
  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        border: `1px ${draftLike || isPending(d.health) ? "dashed" : "solid"} ${tint(pc, draftLike ? 0.65 : 0.45)}`,
        borderRadius: t.radiusLg,
        background: draftLike || d.protocol === "simulated" || d.protocol === "loopback" || d.protocol === "gpio"
          ? t.roleEndpointBg
          : tint(pc, 0.08),
        boxShadow: d.dimmed || draftLike || isPending(d.health) ? "none" : t.shadow,
        overflow: "hidden",
        opacity: d.dimmed ? 0.32 : isPending(d.health) ? 0.9 : 1,
        transition: `box-shadow ${t.ease}, opacity ${t.ease}`,
      }}
    >
      <NodeToolbar isVisible={hover} position={Position.Top}>
        <HoverCard
          title={d.name}
          rows={[
            ["protocol", protocolName(d.protocol)],
            ["role", d.role],
            ["health", d.health],
            ...connectorRows(d),
            ["detail", d.detail],
          ]}
        />
      </NodeToolbar>
      {isComm && <Handle type="source" position={Position.Bottom} style={PORT_STYLE} />}
      <div style={{ position: "relative", flex: hasSlaves ? "0 0 30px" : 1, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 4, padding: "3px 7px" }}>
        <strong style={{ fontSize: 11, fontWeight: 700, color: t.text, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {protocolName(d.protocol)}
        </strong>
        {(d.health === "disabled" || draftLike) && (
          <StatusPill health={d.health} label={draftLike ? "DRAFT" : undefined} tone={draftLike ? t.protocolMuted : undefined} />
        )}
        <span
          className={!draftLike && d.health === "connected" ? "trust-dot trust-dot--live" : "trust-dot"}
          title={d.health}
          style={{
            position: "absolute",
            top: 5,
            right: 6,
            width: 7,
            height: 7,
            borderRadius: "50%",
            background: statusTone,
            boxShadow: `0 0 0 2.5px ${tint(statusTone, 0.16)}`,
          }}
        />
      </div>
      <div
        style={{
          background: tint(pc, 0.16),
          color: t.text,
          fontSize: 8.5,
          fontWeight: 600,
          letterSpacing: 0.4,
          textAlign: "center",
          padding: "2px 3px",
          borderTop: `2px solid ${pc}`,
          textTransform: "uppercase",
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {draftLike ? "DRAFT" : roleWord(d.protocol, d.role)}
      </div>
      {hasSlaves && (
        <div style={{ flex: 1, overflow: "hidden", background: t.canvas }}>
          {slaves.map((s) => (
            <div
              key={s.id}
              title={`${s.name}${s.channels ? ` · ${s.channels} ch` : ""}${s.detail ? ` · ${s.detail}` : ""}`}
              style={{ display: "flex", alignItems: "center", gap: 3, height: 13, padding: "0 5px", borderTop: `1px solid ${t.borderSubtle}` }}
            >
              <span style={{ flex: "none", width: 5, height: 5, borderRadius: "50%", background: healthColor(s.health ?? "") }} />
              <span style={{ flex: 1, minWidth: 0, fontSize: 8, color: t.textMuted, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {s.model ?? s.name}
              </span>
              <span style={{ flex: "none", fontSize: 7.5, color: t.textSubtle }}>·{s.slot}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
});
EndpointNode.displayName = "EndpointNode";

export const ExternalNode = memo(({ data }: NodeProps) => {
  const d = data as ExternalNodeData;
  const [hover, setHover] = useState(false);
  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
        border: `1px dashed ${t.roleExternalBorder}`,
        borderRadius: t.radius,
        background: t.roleExternalBg,
        padding: "0 12px",
        opacity: d.dimmed ? 0.32 : 1,
        transition: `opacity ${t.ease}`,
      }}
    >
      <NodeToolbar isVisible={hover} position={Position.Top}>
        <HoverCard title={d.label} rows={[["presents", d.sub], ["scope", "external system"]]} />
      </NodeToolbar>
      <Handle type="target" position={Position.Top} style={PORT_STYLE} />
      <Handle type="source" position={Position.Bottom} style={PORT_STYLE} />
      <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
        <span style={{ flex: "none", display: "flex", color: t.textMuted }}>{ICONS.external}</span>
        <strong
          title={d.label}
          style={{
            flex: 1,
            minWidth: 0,
            fontSize: 12,
            fontWeight: 600,
            color: t.text,
            lineHeight: 1.25,
            display: "-webkit-box",
            WebkitLineClamp: 2,
            WebkitBoxOrient: "vertical",
            overflow: "hidden",
            overflowWrap: "anywhere",
          }}
        >
          {d.label}
        </strong>
      </div>
      {d.sub && <div style={{ color: t.textMuted, fontSize: 10, marginTop: 2, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{d.sub}</div>}
    </div>
  );
});
ExternalNode.displayName = "ExternalNode";

// §0.4 empty slot: a dashed ghost cell the user clicks in Edit mode to add into that exact spot.
export const SlotNode = memo(({ data }: NodeProps) => {
  const d = data as SlotNodeData;
  const { onPickSlot } = useEditMode();
  const actionLabel = d.slot?.add === "runtime" ? "Set up" : "Add";
  const objectLabel =
    d.slot?.add === "device"
      ? "connection"
      : d.slot?.add === "host"
        ? "host"
        : "runtime";
  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        onPickSlot(d.slot as AddSlotRequest);
      }}
      title={d.label}
      style={slotStyle}
    >
      <span style={{ fontSize: 17, lineHeight: 1, color: t.accent }}>+</span>
      <span style={{ fontSize: 10.5, color: t.text, fontWeight: 650, textAlign: "center", lineHeight: 1.12 }}>
        {actionLabel}
      </span>
      <span style={{ fontSize: 9, color: t.textMuted, textAlign: "center", lineHeight: 1.12 }}>
        {objectLabel}
      </span>
    </button>
  );
});
SlotNode.displayName = "SlotNode";

const slotStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  gap: 3,
  border: `1px dashed ${tint(t.accent, 0.5)}`,
  borderRadius: t.radius,
  background: tint(t.accent, 0.06),
  cursor: "pointer",
  padding: 4,
  transition: `background ${t.ease}, border-color ${t.ease}`,
};

export const nodeTypes = {
  host: HostNode,
  container: ContainerNode,
  runtime: RuntimeNode,
  endpoint: EndpointNode,
  external: ExternalNode,
  bus: BusNode,
  slot: SlotNode,
};
