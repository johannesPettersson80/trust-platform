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
  if (sourcePosition === Position.Bottom || sourcePosition === Position.Top) {
    return 84;
  }
  return 0;
}

function transitionBarStyle(sourcePosition: Position, selected: boolean): React.CSSProperties {
  const sideRouted = sourcePosition === Position.Left || sourcePosition === Position.Right;
  return {
    width: sideRouted ? 3 : 34,
    height: sideRouted ? 34 : 3,
    flex: "none",
    borderRadius: 1,
    background: selected ? t.accent : t.text,
    boxShadow: `0 0 0 2px ${t.canvas}`,
  };
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
      <EdgeLabelRenderer>
        <div
          className="sfc-transition-marker"
          style={{
            position: "absolute",
            transform: `translate(-50%, -50%) translate(${labelX + labelOffset(sourcePosition)}px, ${labelY}px)`,
            display: "flex",
            alignItems: "center",
            gap: 7,
            pointerEvents: "all",
          }}
          title={label || "Transition"}
        >
          <span
            aria-hidden="true"
            className="sfc-transition-bar"
            style={transitionBarStyle(sourcePosition, selected === true)}
          />
          {label && (
            <span
              className="sfc-transition-label"
              style={{
                padding: "2px 6px",
                background: t.canvas,
                color: t.text,
                fontFamily: "var(--vscode-font-family)",
                fontSize: 11,
                fontWeight: 650,
                lineHeight: 1.2,
                maxWidth: 156,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                boxShadow: `0 0 0 2px ${t.canvas}`,
              }}
            >
              {label}
            </span>
          )}
        </div>
      </EdgeLabelRenderer>
    </>
  );
});

TransitionEdge.displayName = "TransitionEdge";
