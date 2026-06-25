import React, { memo, useState } from "react";
import { Handle, NodeToolbar, Position, type NodeProps } from "@xyflow/react";
import { BusNode } from "./BusNode";
import { useEditMode, type AddSlotRequest } from "./editMode";
import { t, tint } from "./theme";
import type {
  ContainerNodeData,
  EndpointNodeData,
  ExternalNodeData,
  HostNodeData,
  RuntimeNodeData,
  SlotNodeData,
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

// Protocol identity colours the wire + the endpoint role band only — never floods a node body.
// Mid-tone hues so the band reads with dark text on both light and dark themes.
const PROTOCOL_COLORS: Record<string, string> = {
  modbus_tcp: "#5b9bd5",
  modbus: "#5b9bd5",
  mqtt: "#d29152",
  opcua: "#3bae9a",
  opcua_client: "#3bae9a",
  ads: "#d2756f",
  ads_server: "#d2756f",
  mesh: "#5cb46c",
  runtime_cloud: "#b39bef",
  federation: "#b39bef",
  realtime: "#b39bef",
  realtime_t0: "#b39bef",
  gpio: "#5aa6a0",
  ethercat: "#b07cc6",
};

export function protocolColor(protocol: string): string {
  return PROTOCOL_COLORS[protocol] ?? "#6b7480";
}

const PROTOCOL_LABELS: Record<string, string> = {
  modbus_tcp: "MB",
  modbus: "MB",
  mqtt: "MQ",
  opcua: "UA",
  opcua_client: "UA",
  ads: "ADS",
  ads_server: "ADS",
  mesh: "ME",
  runtime_cloud: "FE",
  federation: "FE",
  realtime: "T0",
  realtime_t0: "T0",
  gpio: "IO",
  ethercat: "EC",
  web: "WB",
  discovery: "DS",
  simulated: "SM",
  loopback: "LB",
};

export function protocolBadgeLabel(protocol: string): string {
  return PROTOCOL_LABELS[protocol] ?? protocol.replace(/_/g, "").slice(0, 2).toUpperCase();
}

// §8: spell the protocol (no cryptic 2-letter tile). Readable display names.
const PROTOCOL_NAMES: Record<string, string> = {
  modbus_tcp: "Modbus",
  modbus: "Modbus",
  ethercat: "EtherCAT",
  opcua: "OPC UA server",
  opcua_client: "OPC UA client",
  ads: "ADS",
  ads_server: "ADS",
  mqtt: "MQTT",
  mesh: "Mesh",
  discovery: "Discovery",
  realtime: "Realtime",
  realtime_t0: "Realtime",
  runtime_cloud: "Federation",
  federation: "Federation",
  gpio: "GPIO",
  simulated: "Simulated",
  loopback: "Loopback",
  web: "Web",
};

export function protocolName(protocol: string): string {
  return PROTOCOL_NAMES[protocol] ?? protocol.replace(/_/g, " ");
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
const PENDING_STATES = new Set(["pending", "not_configured", "not_in_build", "unknown"]);
function isPending(health: string): boolean {
  return PENDING_STATES.has(health);
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

// A node card surface. Hairline border + soft elevation (theme-aware); dashed + dimmed when pending.
function cardStyle(health: string, opts: { raised?: boolean; radius?: number } = {}): React.CSSProperties {
  const pending = isPending(health);
  return {
    width: "100%",
    height: "100%",
    border: `1px ${pending ? "dashed" : "solid"} ${t.border}`,
    borderRadius: opts.radius ?? t.radius,
    background: opts.raised ? t.surfaceRaised : t.surface,
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
    case "stopped":
      return "Stopped";
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
    <div onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)} style={cardStyle(d.health, { radius: t.radiusLg })}>
      <NodeToolbar isVisible={hover} position={Position.Top}>
        <HoverCard
          title={d.label}
          rows={[["address", d.sub], ["health", d.health], ["runtimes", String(d.runtimeCount)], ["endpoints", String(d.endpointCount)]]}
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

export const RuntimeNode = memo(({ data }: NodeProps) => {
  const d = data as RuntimeNodeData;
  const [hover, setHover] = useState(false);
  return (
    <div onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)} style={cardStyle(d.health, { raised: true })}>
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
        <div style={{ display: "flex" }}>
          <StatusPill health={d.health} />
        </div>
      </div>
    </div>
  );
});
RuntimeNode.displayName = "RuntimeNode";

// Layout (app-icon style): protocol name on top, role in a coloured band below.
export const EndpointNode = memo(({ data }: NodeProps) => {
  const d = data as EndpointNodeData;
  const pc = protocolColor(d.protocol);
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
        border: `1px ${isPending(d.health) ? "dashed" : "solid"} ${t.border}`,
        borderRadius: t.radiusLg,
        background: t.surface,
        boxShadow: d.dimmed || isPending(d.health) ? "none" : t.shadow,
        overflow: "hidden",
        opacity: d.dimmed ? 0.32 : isPending(d.health) ? 0.9 : 1,
        transition: `box-shadow ${t.ease}, opacity ${t.ease}`,
      }}
    >
      <NodeToolbar isVisible={hover} position={Position.Top}>
        <HoverCard
          title={d.name}
          rows={[["protocol", protocolName(d.protocol)], ["role", d.role], ["health", d.health], ["detail", d.detail]]}
        />
      </NodeToolbar>
      {isComm && <Handle type="source" position={Position.Bottom} style={PORT_STYLE} />}
      <div style={{ position: "relative", flex: hasSlaves ? "0 0 30px" : 1, display: "flex", alignItems: "center", justifyContent: "center", padding: "3px 7px" }}>
        <strong style={{ fontSize: 11, fontWeight: 700, color: t.text, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {protocolName(d.protocol)}
        </strong>
        <span
          className={d.health === "connected" ? "trust-dot trust-dot--live" : "trust-dot"}
          title={d.health}
          style={{
            position: "absolute",
            top: 5,
            right: 6,
            width: 7,
            height: 7,
            borderRadius: "50%",
            background: healthColor(d.health),
            boxShadow: `0 0 0 2.5px ${tint(healthColor(d.health), 0.16)}`,
          }}
        />
      </div>
      <div
        style={{
          background: pc,
          color: "#0b0e14",
          fontSize: 8.5,
          fontWeight: 800,
          letterSpacing: 0.3,
          textAlign: "center",
          padding: "2px 3px",
          textTransform: "uppercase",
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {roleWord(d.protocol, d.role)}
      </div>
      {hasSlaves && (
        <div style={{ flex: 1, overflow: "hidden", background: t.canvas }}>
          {slaves.map((s) => (
            <div
              key={s.id}
              title={`${s.name}${s.channels ? ` · ${s.channels} ch` : ""}${s.detail ? ` — ${s.detail}` : ""}`}
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
        border: `1px dashed ${t.border}`,
        borderRadius: t.radius,
        background: t.surface,
        padding: "0 12px",
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
  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        onPickSlot(d.slot as AddSlotRequest);
      }}
      title={`Add ${d.label}`}
      style={slotStyle}
    >
      <span style={{ fontSize: 17, lineHeight: 1, color: t.accent }}>+</span>
      <span style={{ fontSize: 10, color: t.textMuted, textAlign: "center", lineHeight: 1.2 }}>{d.label}</span>
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
