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
  const handleDoubleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (data.onToggleBreakpoint) {
      data.onToggleBreakpoint();
    }
  };

  // IEC 61131-3: Simple rectangular steps
  const isInitial = data.type === "initial";
  const isFinal = data.type === "final";

  const borderColor = selected
    ? t.accent
    : data.isCurrentDebugStep
    ? t.warn
    : t.text;

  const backgroundColor = data.isActive
    ? t.ok
    : data.isCurrentDebugStep
    ? tint(t.warn, 0.2)
    : t.canvas;

  const textColor = data.isActive
    ? t.onAccent
    : t.text;
  
  const borderWidth = data.isActive ? "3px" : "2px";

  // Simple rectangular box per IEC 61131-3
  const stepStyle: React.CSSProperties = {
    width: "200px",
    minHeight: "56px",
    padding: "10px 14px",
    border: isInitial
      ? `4px double ${borderColor}`
      : `${borderWidth} solid ${borderColor}`,
    borderRadius: "2px",
    background: backgroundColor,
    color: textColor,
    fontFamily: "var(--vscode-font-family)",
    fontSize: "13px",
    fontWeight: data.isActive ? 700 : isInitial ? 600 : 500,
    textAlign: "center",
    position: "relative",
    cursor: "pointer",
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    boxSizing: "border-box",
    boxShadow: data.isActive
      ? `0 0 0 3px ${borderColor}, 0 4px 12px ${tint(t.ok, 0.4)}`
      : selected
      ? `0 0 0 2px ${t.accent}`
      : "none",
  };

  // Final step: double bottom border
  if (isFinal) {
    stepStyle.borderBottom = `4px double ${borderColor}`;
  }

  return (
    <div
      style={stepStyle}
      onDoubleClick={handleDoubleClick}
      title="Double-click to toggle breakpoint"
    >
      {/* Breakpoint indicator */}
      {data.hasBreakpoint && (
        <div
          style={{
            position: "absolute",
            top: "-8px",
            left: "-8px",
            width: "16px",
            height: "16px",
            borderRadius: "50%",
            background: t.breakpoint,
            border: `2px solid ${t.canvas}`,
            boxShadow: `0 0 4px ${tint(t.breakpoint, 0.6)}`,
            zIndex: 10,
          }}
          title="Breakpoint"
        />
      )}

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
