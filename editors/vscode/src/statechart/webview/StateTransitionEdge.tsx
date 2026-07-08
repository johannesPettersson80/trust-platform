import React, { memo } from "react";
import {
  BaseEdge,
  EdgeLabelRenderer,
  EdgeProps,
  Position,
  getSmoothStepPath,
} from "@xyflow/react";
import type { StateChartEdge } from "./types";
import { t } from "../../webview/theme";

function labelTranslateY(sourcePosition: Position, targetPosition: Position): number {
  if (sourcePosition === Position.Left && targetPosition === Position.Right) {
    return -130;
  }
  if (sourcePosition === Position.Bottom && targetPosition === Position.Top) {
    return 0;
  }
  return 0;
}

export const STATE_TRANSITION_EDGE = "stateTransition";

export const StateTransitionEdge = memo((props: EdgeProps<StateChartEdge>) => {
  const {
    data,
    label,
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
  const text = data?.event || (typeof label === "string" ? label : undefined);

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
      {text && (
        <EdgeLabelRenderer>
          <div
            className="statechart-transition-label"
            style={{
              position: "absolute",
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY + labelTranslateY(sourcePosition, targetPosition)}px)`,
              padding: "3px 7px",
              border: `1px solid ${t.border}`,
              borderRadius: t.radius,
              background: t.surface,
              color: t.text,
              fontFamily: "var(--vscode-font-family)",
              fontSize: 10,
              fontWeight: 700,
              letterSpacing: 0.2,
              lineHeight: 1.2,
              maxWidth: 120,
              overflow: "hidden",
              pointerEvents: "all",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              boxShadow: t.shadow,
            }}
            title={text}
          >
            {text}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
});

StateTransitionEdge.displayName = "StateTransitionEdge";
