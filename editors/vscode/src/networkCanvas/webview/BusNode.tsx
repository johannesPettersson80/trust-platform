import React, { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { t, tint } from "./theme";

export interface BusNodeData extends Record<string, unknown> {
  label: string;
  color: string;
  draft?: boolean;
  showLabel?: boolean;
  handles: Array<{ id: string; x: number }>; // x = px offset from the bus's left edge
}

// A horizontal bus-bar: peers drop straight down onto it (T-junctions), so the
// connections MERGE into one trunk instead of N point-to-point wires (§0.2/§4.4).
export const BusNode = memo(({ data }: NodeProps) => {
  const d = data as BusNodeData;
  const tone = d.draft ? t.protocolMuted : d.color;
  const showLabel = d.showLabel !== false;
  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <div
        style={{
          position: "absolute",
          left: 0,
          right: 0,
          top: "50%",
          height: 4,
          transform: "translateY(-50%)",
          background: tone,
          border: d.draft ? `1px dashed ${t.protocolMuted}` : "none",
          borderRadius: 2,
          boxShadow: `0 0 0 3px ${t.canvas}`,
        }}
      />
      {showLabel && (
        <div
          className="trust-edge-label-knockout trust-bus-label"
          style={{
            position: "absolute",
            left: "50%",
            top: -16,
            transform: "translateX(-50%)",
            display: "inline-flex",
            alignItems: "center",
            gap: 5,
            fontSize: 9.5,
            fontWeight: 700,
            color: t.textMuted,
            whiteSpace: "nowrap",
            // Opaque knockout so peer wires dropping onto the bus never run through the label text.
            background: t.surface,
            border: `1px solid ${d.draft ? t.protocolMuted : t.border}`,
            boxShadow: `0 0 0 4px ${t.canvas}`,
            padding: "1px 5px",
            borderRadius: 3,
          }}
        >
          <span>{d.label}</span>
          {d.draft && (
            <span
              className="trust-bus-draft-chip"
              style={{
                color: t.protocolMuted,
                background: tint(t.protocolMuted, 0.14),
                border: `1px solid ${tint(t.protocolMuted, 0.35)}`,
                borderRadius: t.pill,
                padding: "0 4px",
                fontSize: 8,
                fontWeight: 800,
                lineHeight: 1.35,
              }}
            >
              DRAFT
            </span>
          )}
        </div>
      )}
      {d.handles.map((h) => (
        <Handle
          key={h.id}
          id={h.id}
          type="target"
          position={Position.Top}
          style={{ left: h.x, top: "50%", background: tone, width: 8, height: 8, borderRadius: 2, border: "none" }}
        />
      ))}
    </div>
  );
});
BusNode.displayName = "BusNode";
