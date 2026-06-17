import React from "react";
import { healthColor, protocolColor, protocolName, roleWord } from "./nodes";

// §4 D: single-click a node → a persistent, read-only details panel (distinct from the
// add-device setup form). Reads the clicked node's own data — no extension round-trip —
// and shows a type-appropriate superset of the hover card, plus a Focus action.

export interface InspectorNode {
  id: string;
  type?: string;
  data: Record<string, unknown>;
}

interface Props {
  node: InspectorNode;
  onClose: () => void;
  onFocus: (nodeId: string) => void;
}

interface View {
  title: string;
  kindLabel: string;
  accent?: string;
  health?: string;
  rows: Array<[string, string]>;
}

function str(value: unknown): string {
  return value === undefined || value === null ? "" : String(value);
}

function viewFor(node: InspectorNode): View {
  const d = node.data;
  switch (node.type) {
    case "endpoint": {
      const protocol = str(d.protocol);
      return {
        title: protocolName(protocol),
        kindLabel: str(d.kind) === "field" ? "Field endpoint" : "Communication endpoint",
        accent: protocolColor(protocol),
        health: str(d.health),
        rows: [
          ["name", str(d.name)],
          ["protocol", protocolName(protocol)],
          ["role", roleWord(protocol, str(d.role))],
          ["kind", str(d.kind)],
          ["status", str(d.health)],
          ["detail", str(d.detail)],
        ],
      };
    }
    case "runtime":
      return {
        title: str(d.label),
        kindLabel: "Runtime",
        health: str(d.health),
        rows: [
          ["mode", str(d.mode)],
          ["status", str(d.health)],
          ["endpoints", str(d.endpointCount)],
          ["container", str(d.container)],
          ["detail", str(d.detail)],
        ],
      };
    case "host":
      return {
        title: str(d.label),
        kindLabel: "Host",
        health: str(d.health),
        rows: [
          ["address", str(d.sub)],
          ["status", str(d.health)],
          ["runtimes", str(d.runtimeCount)],
          ["endpoints", str(d.endpointCount)],
        ],
      };
    case "container":
      return {
        title: str(d.label),
        kindLabel: "Container",
        rows: [
          ["image", str(d.image)],
          ["status", str(d.status)],
        ],
      };
    case "external":
      return {
        title: str(d.label),
        kindLabel: "External system",
        rows: [
          ["presents", str(d.sub)],
          ["scope", "external — configured on our side, on the relevant endpoint"],
        ],
      };
    default:
      return {
        title: str(d.label) || str(d.name) || node.id,
        kindLabel: node.type ?? "node",
        rows: [],
      };
  }
}

const PANEL_STYLE: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 320,
  maxWidth: "92vw",
  background: "rgba(18,21,28,.98)",
  borderLeft: "1px solid #2a2f3a",
  boxShadow: "-18px 0 50px rgba(0,0,0,.45)",
  zIndex: 8,
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
};

export function NodeInspector({ node, onClose, onFocus }: Props) {
  const view = viewFor(node);
  const rows = view.rows.filter(([, v]) => v);
  return (
    <aside style={PANEL_STYLE} aria-label="Node details">
      <header style={{ display: "flex", alignItems: "center", gap: 9, padding: "12px 14px", borderBottom: "1px solid #2a2f3a" }}>
        {view.accent && (
          <span style={{ flex: "none", width: 10, height: 10, borderRadius: 3, background: view.accent }} />
        )}
        <div style={{ flex: 1, minWidth: 0 }}>
          <strong style={{ display: "block", fontSize: 14, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
            {view.title}
          </strong>
          <span style={{ fontSize: 10.5, color: "#7f8794", textTransform: "uppercase", letterSpacing: 0.4 }}>
            {view.kindLabel}
          </span>
        </div>
        {view.health && (
          <span
            title={view.health}
            style={{ flex: "none", width: 10, height: 10, borderRadius: "50%", background: healthColor(view.health), boxShadow: `0 0 0 2px ${healthColor(view.health)}30` }}
          />
        )}
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </header>

      <div style={{ flex: 1, overflow: "auto", padding: 14 }}>
        {rows.length === 0 ? (
          <p style={{ color: "#7f8794", fontSize: 12 }}>No further details.</p>
        ) : (
          rows.map(([k, v]) => (
            <div key={k} style={{ display: "flex", gap: 10, fontSize: 12, lineHeight: 1.55, marginBottom: 7 }}>
              <span style={{ color: "#7f8794", flex: "none", minWidth: 74 }}>{k}</span>
              <span style={{ color: "#cfd6e0", overflowWrap: "anywhere" }}>{v}</span>
            </div>
          ))
        )}
      </div>

      <footer style={{ display: "flex", gap: 8, padding: 12, borderTop: "1px solid #2a2f3a" }}>
        <button onClick={() => onFocus(node.id)} style={{ ...primaryBtn, flex: 1 }}>
          Focus on canvas
        </button>
      </footer>
    </aside>
  );
}

const iconBtn: React.CSSProperties = {
  border: "none",
  background: "transparent",
  color: "#949cab",
  fontSize: 14,
  cursor: "pointer",
};
const primaryBtn: React.CSSProperties = {
  border: "1px solid #2f81f7",
  background: "rgba(47,129,247,.16)",
  color: "#cfe0ff",
  borderRadius: 7,
  padding: "8px 13px",
  fontSize: 12,
  fontWeight: 600,
  cursor: "pointer",
};
