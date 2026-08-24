import React, { useState } from "react";
import {
  BaseEdge,
  EdgeLabelRenderer,
  getSmoothStepPath,
  type EdgeProps,
} from "@xyflow/react";
import { t } from "./theme";
import { healthStatusLabel } from "./statusPresentation";

// §8: edges are orthogonal with rounded corners + a casing in the canvas-bg colour
// so the wire reads clearly above nodes and the dotted background; protocol-coloured.
// No text label — the protocol + role are named on BOTH endpoint nodes.
export function CasedEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  markerStart,
  markerEnd,
}: EdgeProps) {
  // Each wire gets its own lane (centerY) so horizontal runs don't overlap.
  const centerY = typeof data?.centerY === "number" ? (data.centerY as number) : undefined;
  const [path, labelX, labelY] = getSmoothStepPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    borderRadius: 8,
    ...(centerY !== undefined ? { centerY } : {}),
  });
  const color = (data?.color as string) ?? t.accent;
  const dashed = data?.dashed ? "5 4" : undefined;
  const dimmed = Boolean(data?.dimmed);
  const [showHealthDetail, setShowHealthDetail] = useState(false);
  const status = typeof data?.status === "string" ? data.status.trim() : "";
  const detail = typeof data?.detail === "string" ? data.detail.trim() : "";
  const hasHealthDetail = status.length > 0 || detail.length > 0;
  const statusLabel = status.length > 0 ? healthStatusLabel(status) : "Link";
  const healthDetail = detail.length > 0 ? `${statusLabel} — ${detail}` : statusLabel;

  return (
    <>
      <path
        d={path}
        fill="none"
        stroke={t.canvas}
        strokeWidth={5}
        strokeLinecap="round"
        opacity={dimmed ? 0.45 : 1}
      />
      <BaseEdge
        id={id}
        path={path}
        markerStart={markerStart}
        markerEnd={markerEnd}
        style={{ stroke: color, strokeWidth: 1.7, strokeDasharray: dashed, opacity: dimmed ? 0.32 : 1 }}
      />
      {hasHealthDetail && (
        <path
          d={path}
          fill="none"
          stroke="transparent"
          strokeWidth={14}
          pointerEvents="stroke"
          tabIndex={0}
          role="img"
          aria-label={healthDetail}
          data-link-health=""
          onMouseEnter={() => setShowHealthDetail(true)}
          onMouseLeave={() => setShowHealthDetail(false)}
          onFocus={() => setShowHealthDetail(true)}
          onBlur={() => setShowHealthDetail(false)}
        >
          <title>{healthDetail}</title>
        </path>
      )}
      {hasHealthDetail && showHealthDetail && (
        <EdgeLabelRenderer>
          <div
            data-link-health-detail={id}
            role="status"
            style={{
              position: "absolute",
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
              maxWidth: 280,
              padding: "5px 8px",
              border: `1px solid ${color}`,
              borderRadius: t.radius,
              background: t.overlay,
              color: t.text,
              boxShadow: t.shadow,
              fontSize: 11,
              lineHeight: 1.35,
              pointerEvents: "none",
              zIndex: 20,
            }}
          >
            {healthDetail}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}

export const edgeTypes = { cased: CasedEdge };
