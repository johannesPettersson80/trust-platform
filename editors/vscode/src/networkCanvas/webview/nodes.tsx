import React, { memo, useState } from "react";
import { Handle, NodeToolbar, Position, type NodeProps } from "@xyflow/react";
import { BusNode } from "./BusNode";
import { useEditMode, type AddSlotRequest } from "./editMode";
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

// §8 design tokens: status colours (label-paired, colour-blind safe via the pill text).
const STATUS_GREEN = "#46c265";
const STATUS_AMBER = "#e0b341";
const STATUS_RED = "#f0584f";
const STATUS_GREY = "#6b7480";

export function healthColor(health: string): string {
  switch (health) {
    case "connected":
      return STATUS_GREEN;
    case "degraded":
      return STATUS_AMBER;
    case "error":
    case "runtime_unreachable":
      return STATUS_RED;
    default:
      return STATUS_GREY; // not_configured / configured_policy / pending / simulate / unknown
  }
}

// §8 design tokens: colour means status + protocol-on-wire only. Protocol identity colours
// the wire and the endpoint badge — never floods the node body.
const PROTOCOL_COLORS: Record<string, string> = {
  modbus_tcp: "#5b9bd5",
  modbus: "#5b9bd5",
  mqtt: "#d29152",
  opcua: "#3bae9a",
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
  return PROTOCOL_COLORS[protocol] ?? STATUS_GREY;
}

const PROTOCOL_LABELS: Record<string, string> = {
  modbus_tcp: "MB",
  modbus: "MB",
  mqtt: "MQ",
  opcua: "UA",
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
  opcua: "OPC UA",
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

// Ports are square/neutral nubs (NOT circles) so they never read as a status dot.
const PORT_STYLE: React.CSSProperties = {
  background: "#5a6472",
  width: 8,
  height: 8,
  borderRadius: 2,
  border: "none",
};

// §7 honesty: unproven/pending config renders ghost/dashed and never green.
const PENDING_STATES = new Set(["pending", "not_configured", "not_in_build", "unknown"]);
function isPending(health: string): boolean {
  return PENDING_STATES.has(health);
}

// On-canvas status is a dot (§4.3); the full state word lives in the hover card + inspector.
function StatusDot({ health }: { health: string }) {
  const c = healthColor(health);
  return (
    <span
      title={health}
      style={{ flex: "none", width: 9, height: 9, borderRadius: "50%", background: c, boxShadow: `0 0 0 2px ${c}30` }}
    />
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
        background: "rgba(18,21,28,.98)",
        border: "1px solid #2a2f3a",
        borderRadius: 8,
        boxShadow: "0 14px 40px rgba(0,0,0,.5)",
        padding: "9px 11px",
      }}
    >
      <div style={{ fontSize: 12, fontWeight: 700, marginBottom: 6 }}>{title}</div>
      {rows
        .filter(([, v]) => v)
        .map(([k, v]) => (
          <div key={k} style={{ display: "flex", gap: 10, fontSize: 11, lineHeight: 1.5 }}>
            <span style={{ color: "#7f8794", flex: "none", minWidth: 62 }}>{k}</span>
            <span style={{ color: "#cfd6e0", overflowWrap: "anywhere" }}>{v}</span>
          </div>
        ))}
    </div>
  );
}

// Edit-mode "+" insertion affordance (shown on a runtime when edit mode is on).
const modeBadgeStyle: React.CSSProperties = {
  flex: "none",
  fontSize: 9.5,
  fontWeight: 700,
  color: "#cfe0ff",
  border: "1px solid #343b47",
  borderRadius: 5,
  padding: "1px 6px",
  textTransform: "uppercase",
};

export const HostNode = memo(({ data }: NodeProps) => {
  const d = data as HostNodeData;
  const c = healthColor(d.health);
  const [hover, setHover] = useState(false);
  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        width: "100%",
        height: "100%",
        border: `1px ${isPending(d.health) ? "dashed" : "solid"} ${c}55`,
        borderRadius: 12,
        background: "linear-gradient(180deg,rgba(29,33,42,.55),rgba(16,18,24,.6))",
        boxShadow: "0 18px 50px rgba(0,0,0,.35)",
        opacity: isPending(d.health) ? 0.82 : 1,
      }}
    >
      <NodeToolbar isVisible={hover} position={Position.Top}>
        <HoverCard
          title={d.label}
          rows={[["address", d.sub], ["health", d.health], ["runtimes", String(d.runtimeCount)], ["endpoints", String(d.endpointCount)]]}
        />
      </NodeToolbar>
      <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "12px 14px" }}>
        <span style={{ display: "flex", color: "#9aa6b6" }}>{ICONS.host}</span>
        <strong style={{ flex: 1, minWidth: 0, fontSize: 14, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {d.label}
        </strong>
        <StatusDot health={d.health} />
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
      style={{ width: "100%", height: "100%", border: "1px dashed #3a4150", borderRadius: 10, background: "rgba(20,24,32,.4)" }}
    >
      <NodeToolbar isVisible={hover} position={Position.Top}>
        <HoverCard title={d.label} rows={[["image", d.image], ["status", d.status]]} />
      </NodeToolbar>
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 11px", color: "#cfe0ff" }}>
        <span style={{ display: "flex", color: "#9aa6b6" }}>{ICONS.container}</span>
        <strong style={{ flex: 1, minWidth: 0, fontSize: 12, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{d.label}</strong>
        <span style={{ flex: "none", color: "#949cab", fontSize: 10 }}>{d.status}</span>
      </div>
    </div>
  );
});
ContainerNode.displayName = "ContainerNode";

export const RuntimeNode = memo(({ data }: NodeProps) => {
  const d = data as RuntimeNodeData;
  const c = healthColor(d.health);
  const [hover, setHover] = useState(false);
  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        width: "100%",
        height: "100%",
        border: `1px ${isPending(d.health) ? "dashed" : "solid"} ${c}66`,
        borderRadius: 10,
        background: "linear-gradient(180deg,rgba(33,38,49,.92),rgba(22,26,34,.94))",
        opacity: isPending(d.health) ? 0.85 : 1,
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
      <div style={{ display: "flex", alignItems: "center", gap: 9, padding: "10px 13px" }}>
        <span style={{ display: "flex", color: "#9aa6b6" }}>{ICONS.runtime}</span>
        <strong style={{ flex: 1, minWidth: 0, fontSize: 13, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{d.label}</strong>
        {d.container && (
          <span title={`container: ${d.container}`} style={{ flex: "none", display: "inline-flex", alignItems: "center", color: "#9aa6b6", border: "1px solid #343b47", borderRadius: 5, padding: "2px 5px" }}>
            {ICONS.container}
          </span>
        )}
        <span style={modeBadgeStyle}>{d.mode}</span>
        <StatusDot health={d.health} />
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
        border: `1px ${isPending(d.health) ? "dashed" : "solid"} #2a2f3a`,
        borderRadius: 8,
        background: "rgba(15,18,24,.96)",
        overflow: "hidden",
        opacity: d.dimmed ? 0.32 : isPending(d.health) ? 0.85 : 1,
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
        <strong style={{ fontSize: 11, fontWeight: 800, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {protocolName(d.protocol)}
        </strong>
        <span
          title={d.health}
          style={{
            position: "absolute",
            top: 5,
            right: 6,
            width: 8,
            height: 8,
            borderRadius: "50%",
            background: healthColor(d.health),
            boxShadow: `0 0 0 2px ${healthColor(d.health)}30`,
          }}
        />
      </div>
      <div
        style={{
          background: pc,
          color: "#0c111a",
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
        <div style={{ flex: 1, overflow: "hidden", background: "rgba(10,13,18,.6)" }}>
          {slaves.map((s) => (
            <div
              key={s.id}
              title={`${s.name}${s.channels ? ` · ${s.channels} ch` : ""}${s.detail ? ` — ${s.detail}` : ""}`}
              style={{ display: "flex", alignItems: "center", gap: 3, height: 13, padding: "0 5px", borderTop: "1px solid #161b24" }}
            >
              <span style={{ flex: "none", width: 5, height: 5, borderRadius: "50%", background: healthColor(s.health ?? "") }} />
              <span style={{ flex: 1, minWidth: 0, fontSize: 8, color: "#c4ccd8", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {s.model ?? s.name}
              </span>
              <span style={{ flex: "none", fontSize: 7.5, color: "#6a7280" }}>·{s.slot}</span>
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
        border: "1px dashed #4a5263",
        borderRadius: 10,
        background: "rgba(20,24,32,.7)",
        padding: "0 12px",
      }}
    >
      <NodeToolbar isVisible={hover} position={Position.Top}>
        <HoverCard title={d.label} rows={[["presents", d.sub], ["scope", "external system"]]} />
      </NodeToolbar>
      <Handle type="target" position={Position.Top} style={PORT_STYLE} />
      <Handle type="source" position={Position.Bottom} style={PORT_STYLE} />
      <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
        <span style={{ display: "flex", color: "#9aa6b6" }}>{ICONS.external}</span>
        <strong style={{ flex: 1, minWidth: 0, fontSize: 12, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{d.label}</strong>
      </div>
      {d.sub && <div style={{ color: "#9aa6b6", fontSize: 10, marginTop: 2, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{d.sub}</div>}
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
      <span style={{ fontSize: 17, lineHeight: 1, color: "#5aa9ff" }}>+</span>
      <span style={{ fontSize: 10, color: "#8a93a3", textAlign: "center", lineHeight: 1.2 }}>{d.label}</span>
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
  border: "1px dashed #3a4150",
  borderRadius: 9,
  background: "rgba(90,169,255,.05)",
  cursor: "pointer",
  padding: 4,
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
