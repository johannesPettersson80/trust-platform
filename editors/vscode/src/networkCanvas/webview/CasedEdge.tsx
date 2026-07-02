import React from "react";
import { BaseEdge, getSmoothStepPath, type EdgeProps } from "@xyflow/react";
import { t } from "./theme";

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
  const [path] = getSmoothStepPath({
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
    </>
  );
}

export const edgeTypes = { cased: CasedEdge };
