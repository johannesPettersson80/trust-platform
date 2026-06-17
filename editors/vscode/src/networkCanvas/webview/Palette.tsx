import React, { useMemo, useState } from "react";
import type { CommSchemaResponse } from "../../communication/schemaForm";
import { protocolBadgeLabel, protocolColor } from "./nodes";

// §6.1: left-rail searchable catalog. Drag an item onto a runtime to add it there
// (drop target decides ownership). Drag payload = the protocol id.
export const PROTOCOL_DND_MIME = "application/trust-protocol";

export function Palette({
  schema,
  reachable,
}: {
  schema?: CommSchemaResponse;
  reachable: boolean;
}) {
  const [query, setQuery] = useState("");
  const protocols = useMemo(() => schema?.protocols ?? [], [schema]);
  const q = query.trim().toLowerCase();
  const filtered = protocols.filter(
    (p) => p.title.toLowerCase().includes(q) || p.id.toLowerCase().includes(q)
  );

  return (
    <aside style={PALETTE} aria-label="Device palette">
      <div style={{ padding: "11px 12px", borderBottom: "1px solid #2a2f3a" }}>
        <div style={{ fontSize: 11, fontWeight: 700, color: "#cfd6e0", marginBottom: 8 }}>
          Add to a runtime
        </div>
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search devices"
          style={SEARCH}
        />
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: 8 }}>
        {!reachable ? (
          <p style={{ color: "#7f8794", fontSize: 11, padding: "6px 8px", lineHeight: 1.5 }}>
            Connect an online runtime to load its device catalog.
          </p>
        ) : filtered.length === 0 ? (
          <p style={{ color: "#7f8794", fontSize: 11, padding: "6px 8px" }}>No matching devices.</p>
        ) : (
          filtered.map((p) => (
            <div
              key={p.id}
              draggable
              onDragStart={(e) => {
                e.dataTransfer.setData(PROTOCOL_DND_MIME, p.id);
                e.dataTransfer.effectAllowed = "copy";
              }}
              title={`Drag onto a runtime to add ${p.title}`}
              style={ITEM}
            >
              <span style={{ ...BADGE, background: protocolColor(p.id) }}>
                {protocolBadgeLabel(p.id)}
              </span>
              <span style={{ fontSize: 12 }}>{p.title}</span>
            </div>
          ))
        )}
      </div>
      <div style={{ padding: "8px 12px", borderTop: "1px solid #2a2f3a", color: "#6a7280", fontSize: 10, lineHeight: 1.4 }}>
        Drag onto a runtime — or right-click a runtime → Add endpoint.
      </div>
    </aside>
  );
}

const PALETTE: React.CSSProperties = {
  position: "absolute",
  top: 0,
  left: 0,
  bottom: 0,
  width: 210,
  background: "rgba(16,19,26,.96)",
  borderRight: "1px solid #2a2f3a",
  zIndex: 7,
  display: "flex",
  flexDirection: "column",
};
const SEARCH: React.CSSProperties = {
  width: "100%",
  background: "#10141b",
  border: "1px solid #343b47",
  borderRadius: 7,
  color: "#eef1f5",
  padding: "6px 9px",
  fontSize: 12,
};
const ITEM: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 9,
  padding: "8px 9px",
  marginBottom: 6,
  borderRadius: 8,
  border: "1px solid #2a2f3a",
  background: "rgba(29,33,42,.7)",
  cursor: "grab",
  userSelect: "none",
};
const BADGE: React.CSSProperties = {
  fontSize: 9,
  fontWeight: 800,
  color: "#0c0f14",
  borderRadius: 4,
  padding: "2px 5px",
  textTransform: "uppercase",
};
