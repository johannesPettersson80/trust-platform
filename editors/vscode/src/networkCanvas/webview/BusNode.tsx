import React, { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { t } from "./theme";

export interface BusNodeData extends Record<string, unknown> {
  label: string;
  color: string;
  draft?: boolean;
  handles: Array<{ id: string; x: number }>; // x = px offset from the bus's left edge
}

// A horizontal bus-bar: peers drop straight down onto it (T-junctions), so the
// connections MERGE into one trunk instead of N point-to-point wires (§0.2/§4.4).
export const BusNode = memo(({ data }: NodeProps) => {
  const d = data as BusNodeData;
  const label = d.draft ? `${d.label} · DRAFT` : d.label;
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
          background: d.color,
          border: d.draft ? `1px dashed ${t.border}` : "none",
          borderRadius: 2,
          boxShadow: `0 0 0 3px ${t.canvas}`,
        }}
      />
      <div
        style={{
          position: "absolute",
          left: "50%",
          top: -16,
          transform: "translateX(-50%)",
          fontSize: 9.5,
          fontWeight: 700,
          color: t.textMuted,
          whiteSpace: "nowrap",
          // Opaque knockout so peer wires dropping onto the bus never run through the label text.
          background: t.canvas,
          padding: "1px 5px",
          borderRadius: 3,
        }}
      >
        {label}
      </div>
      {d.handles.map((h) => (
        <Handle
          key={h.id}
          id={h.id}
          type="target"
          position={Position.Top}
          style={{ left: h.x, top: "50%", background: d.color, width: 8, height: 8, borderRadius: 2, border: "none" }}
        />
      ))}
    </div>
  );
});
BusNode.displayName = "BusNode";
