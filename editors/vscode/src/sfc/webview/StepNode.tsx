import React, { memo } from "react";
import { Handle, Position, NodeProps } from "@xyflow/react";
import type { SfcStepNode } from "./types";
import { t, tint } from "../../webview/theme";

export const STEP_TARGET_TOP = "step-target-top";
export const STEP_TARGET_LEFT = "step-target-left";
export const STEP_TARGET_RIGHT = "step-target-right";
export const STEP_SOURCE_BOTTOM = "step-source-bottom";
export const STEP_SOURCE_LEFT = "step-source-left";
export const STEP_SOURCE_RIGHT = "step-source-right";

const baseHandleStyle: React.CSSProperties = {
  background: t.border,
  width: 8,
  height: 8,
  border: `1px solid ${t.canvas}`,
};

/**
 * Custom node component for SFC steps following IEC 61131-3 standard
 */
export const StepNode = memo(({ data, selected }: NodeProps<SfcStepNode>) => {
  // IEC 61131-3: Simple rectangular steps
  const isInitial = data.type === "initial";
  const isFinal = data.type === "final";

  const borderColor = selected ? t.accent : t.text;

  // Simple rectangular box per IEC 61131-3
  const stepStyle: React.CSSProperties = {
    width: "200px",
    minHeight: "56px",
    padding: "10px 14px",
    border: isInitial
      ? `4px double ${borderColor}`
      : `2px solid ${borderColor}`,
    borderRadius: "2px",
    background: t.canvas,
    color: t.text,
    fontFamily: "var(--vscode-font-family)",
    fontSize: "13px",
    fontWeight: isInitial ? 600 : 500,
    textAlign: "center",
    position: "relative",
    cursor: "pointer",
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    boxSizing: "border-box",
    boxShadow: selected ? `0 0 0 2px ${t.accent}` : "none",
  };

  // Final step: double bottom border
  if (isFinal) {
    stepStyle.borderBottom = `4px double ${borderColor}`;
  }

  return (
    <div style={stepStyle}>

      {isInitial && (
        <div
          style={{
            position: "absolute",
            top: 4,
            right: 6,
            padding: "1px 5px",
            border: `1px solid ${tint(borderColor, 0.45)}`,
            borderRadius: 2,
            background: tint(t.canvas, 0.86),
            color: borderColor,
            fontSize: 8,
            fontWeight: 700,
            letterSpacing: 0,
            lineHeight: 1.15,
          }}
        >
          INITIAL
        </div>
      )}

      <Handle
        id={STEP_TARGET_TOP}
        type="target"
        position={Position.Top}
        style={{
          ...baseHandleStyle,
          background: borderColor,
          top: -4,
        }}
      />
      <Handle
        id={STEP_TARGET_LEFT}
        type="target"
        position={Position.Left}
        style={{
          ...baseHandleStyle,
          background: borderColor,
          left: -4,
        }}
      />
      <Handle
        id={STEP_TARGET_RIGHT}
        type="target"
        position={Position.Right}
        style={{
          ...baseHandleStyle,
          background: borderColor,
          right: -4,
        }}
      />

      <div
        style={{
          width: "100%",
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {data.label}
      </div>

      {data.description && (
        <div
          style={{
            fontSize: "10px",
            opacity: 0.7,
            marginTop: "4px",
            width: "100%",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {data.description}
        </div>
      )}

      <Handle
        id={STEP_SOURCE_BOTTOM}
        type="source"
        position={Position.Bottom}
        style={{
          ...baseHandleStyle,
          background: borderColor,
          bottom: -4,
        }}
      />
      <Handle
        id={STEP_SOURCE_LEFT}
        type="source"
        position={Position.Left}
        style={{
          ...baseHandleStyle,
          background: borderColor,
          left: -4,
          top: "62%",
        }}
      />
      <Handle
        id={STEP_SOURCE_RIGHT}
        type="source"
        position={Position.Right}
        style={{
          ...baseHandleStyle,
          background: borderColor,
          right: -4,
          top: "62%",
        }}
      />
    </div>
  );
});

StepNode.displayName = "StepNode";
