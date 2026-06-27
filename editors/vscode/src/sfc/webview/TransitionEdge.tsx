import React, { memo } from "react";
import {
  BaseEdge,
  EdgeLabelRenderer,
  EdgeProps,
  Position,
  getSmoothStepPath,
} from "@xyflow/react";
import type { SfcTransitionEdge } from "./types";
import { t } from "../../webview/theme";

function labelOffset(sourcePosition: Position): number {
  if (sourcePosition === Position.Right) {
    return 56;
  }
  if (sourcePosition === Position.Left) {
    return -56;
  }
  return 0;
}

export const TransitionEdge = memo((props: EdgeProps<SfcTransitionEdge>) => {
  const {
    data,
    markerEnd,
    selected,
    sourcePosition,
    sourceX,
    sourceY,
    style,
    targetPosition,
    targetX,
    targetY,
  } = props;
  const [edgePath, labelX, labelY] = getSmoothStepPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
    borderRadius: 12,
  });
  const label = data?.label || data?.condition;

  return (
    <>
      <BaseEdge
        path={edgePath}
        markerEnd={markerEnd}
        style={{
          stroke: selected ? t.accent : "var(--trust-border)",
          strokeWidth: selected ? 2.5 : 2,
          ...style,
        }}
      />
      {label && (
        <EdgeLabelRenderer>
          <div
            className="sfc-transition-label"
            style={{
              position: "absolute",
              transform: `translate(-50%, -50%) translate(${labelX + labelOffset(sourcePosition)}px, ${labelY}px)`,
              padding: "4px 9px",
              border: `1px solid ${t.border}`,
              borderRadius: t.radius,
              background: t.surface,
              color: t.text,
              fontFamily: "var(--vscode-font-family)",
              fontSize: 11,
              fontWeight: 650,
              lineHeight: 1.2,
              maxWidth: 156,
              overflow: "hidden",
              pointerEvents: "all",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              boxShadow: t.shadow,
            }}
            title={label}
          >
            {label}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
});

TransitionEdge.displayName = "TransitionEdge";
